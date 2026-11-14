//! iOS: the system's own fonts for the interface, so the app carries none of its own for other
//! scripts. CoreText says which font the system draws its UI in (SF Pro) and which it would use
//! for a piece of text in another script (Hiragino for Japanese, PingFang for Chinese, Apple SD
//! Gothic Neo for Korean, SF Georgian …); each file is mapped read-only, so its pages stay the
//! system's clean, droppable file pages, not app memory.
//!
//! Gotcha: the system's own UI fonts can keep their outlines in Apple's `hvgl` table (iOS 26's
//! PingFang UI does), which egui can't draw: every kanji and hanzi came out blank. A font without
//! TrueType or CFF outlines is passed over for the named fonts of [`NAMED`].
#![allow(unsafe_code)] // CoreText calls and a read-only mmap

use std::path::{Path, PathBuf};

use objc2_core_foundation::{CFRange, CFRetained, CFString, CFURL};
use objc2_core_text::{CTFont, CTFontUIFontType, kCTFontURLAttribute};

/// Text in each script the interface may show (the language names among them), with the script
/// it is written in: the system picks the font for each.
const SAMPLES: &[(&str, &str)] = &[
    ("日本語", "Jpan"),
    ("简体中文", "Hans"),
    ("繁體中文（台灣）", "Hant"),
    ("한국어", "Kore"),
    ("ქართული", "Geor"),
    ("ไทย", "Thai"),
    ("العربية", "Arab"),
    ("עברית", "Hebr"),
    ("हिन्दी", "Deva"),
    ("Հայերեն", "Armn"),
];

/// Fonts asked for by name when the system's pick for a script can't be drawn (see the module
/// note): Hiragino Sans for Japanese, PingFang or Heiti for Chinese.
const NAMED: &[(&str, &[&str])] = &[
    ("Jpan", &["HiraginoSans-W3", "HiraginoKakuGothicProN-W3"]),
    ("Hans", &["PingFangSC-Regular", "STHeitiSC-Light", "HiraginoSansGB-W3"]),
    ("Hant", &["PingFangTC-Regular", "STHeitiTC-Light"]),
];

/// Use the system's fonts: its UI font first, the others as fallbacks for their scripts.
pub fn install() {
    // SAFETY: CoreText's documented constructor for the system UI font; size 0 = the default
    let Some(ui) = (unsafe { CTFont::new_ui_font_for_language(CTFontUIFontType::System, 0.0, None) }) else { return };
    if let Some((bytes, index)) = face(&ui) {
        lightcraft_ui_egui::theme::set_primary_font(bytes, index);
    }
    let mut seen: Vec<(PathBuf, u32)> = Vec::new();
    for (text, script) in SAMPLES {
        let s = CFString::from_str(text);
        let range = CFRange { location: 0, length: s.length() };
        // SAFETY: a valid font, string and a range within it
        let picked = unsafe { ui.for_string(&s, range) };
        let named = NAMED.iter().filter(|(sc, _)| sc == script).flat_map(|(_, names)| names.iter()).map(|name| {
            // SAFETY: CoreText's constructor by name; size 0 = the default, no matrix
            unsafe { CTFont::with_name(&CFString::from_str(name), 0.0, std::ptr::null()) }
        });
        let found = std::iter::once(picked).chain(named).find_map(|font| {
            let path = path_of(&font)?;
            let (bytes, index) = face(&font)?;
            drawable(bytes, index, text).then_some((path, bytes, index))
        });
        let Some((path, bytes, index)) = found else {
            log::warn!("no system font the app can draw {script} with");
            continue;
        };
        log::info!("system font for {script}: {} face {index}", path.display());
        if seen.contains(&(path.clone(), index)) {
            continue;
        }
        seen.push((path, index));
        lightcraft_ui_egui::theme::add_script_fallback_font(&format!("system {script}"), bytes, index, script);
    }
}

/// Whether egui can draw `text` with face `index` of `bytes`: TrueType or CFF outlines (not only
/// Apple's `hvgl`), and a glyph for each character.
fn drawable(bytes: &[u8], index: u32, text: &str) -> bool {
    let Ok(f) = ttf_parser::Face::parse(bytes, index) else { return false };
    let outlines = [b"glyf", b"CFF ", b"CFF2"].iter().any(|t| f.raw_face().table(ttf_parser::Tag::from_bytes(t)).is_some());
    // (punctuation and spaces may come from another font)
    outlines && text.chars().filter(|c| c.is_alphabetic()).all(|c| f.glyph_index(c).is_some())
}

/// The file `font` is in.
fn path_of(font: &CTFont) -> Option<PathBuf> {
    // SAFETY: a valid font and CoreText's own attribute name
    let url = unsafe { font.attribute(kCTFontURLAttribute) }?;
    let url: CFRetained<CFURL> = url.downcast().ok()?;
    url.to_file_path()
}

/// `font`'s file, mapped, and which face of it `font` is (a .ttc holds several).
fn face(font: &CTFont) -> Option<(&'static [u8], u32)> {
    let bytes = map(&path_of(font)?)?;
    // SAFETY: a valid font
    let name = unsafe { font.post_script_name() }.to_string();
    let count = ttf_parser::fonts_in_collection(bytes).unwrap_or(1);
    let index = (0..count)
        .find(|&i| {
            ttf_parser::Face::parse(bytes, i).is_ok_and(|f| {
                f.names().into_iter().any(|n| n.name_id == ttf_parser::name_id::POST_SCRIPT_NAME && n.to_string().is_some_and(|s| s == name))
            })
        })
        .unwrap_or(0);
    Some((bytes, index))
}

/// The file at `path`, mapped read-only for the life of the app.
fn map(path: &Path) -> Option<&'static [u8]> {
    let c = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).ok()?;
    // SAFETY: plain libc calls on a NUL-terminated path; the mapping is checked, kept for the
    // process's life (the slice is 'static) and never written: system fonts are on the read-only
    // system volume (or the system's sealed font assets), so they can't change under the mapping
    unsafe {
        let fd = libc::open(c.as_ptr(), libc::O_RDONLY | libc::O_CLOEXEC);
        if fd < 0 {
            return None;
        }
        let mut st: libc::stat = std::mem::zeroed();
        let len = if libc::fstat(fd, &mut st) == 0 { usize::try_from(st.st_size).unwrap_or(0) } else { 0 };
        let p = if len > 0 { libc::mmap(std::ptr::null_mut(), len, libc::PROT_READ, libc::MAP_PRIVATE, fd, 0) } else { libc::MAP_FAILED };
        libc::close(fd);
        if p == libc::MAP_FAILED {
            return None;
        }
        Some(std::slice::from_raw_parts(p.cast::<u8>(), len))
    }
}
