//! iOS: the system's own fonts for the interface, so the app carries none of its own for other
//! scripts. CoreText says which font the system draws its UI in (SF Pro) and which it would use
//! for a piece of text in another script (Hiragino for Japanese, PingFang for Chinese, Apple SD
//! Gothic Neo for Korean, SF Georgian …); each file is mapped read-only, so its pages stay the
//! system's clean, droppable file pages, not app memory.
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

/// Use the system's fonts: its UI font first, the others as fallbacks for their scripts.
pub fn install() {
    // SAFETY: CoreText's documented constructor for the system UI font; size 0 = the default
    let Some(ui) = (unsafe { CTFont::new_ui_font_for_language(CTFontUIFontType::System, 0.0, None) }) else { return };
    if let Some((bytes, index)) = face(&ui) {
        lightcraft_ui_egui::theme::set_primary_font(bytes, index);
    }
    let mut seen: Vec<PathBuf> = Vec::new();
    for (text, script) in SAMPLES {
        let s = CFString::from_str(text);
        let range = CFRange { location: 0, length: s.length() };
        // SAFETY: a valid font, string and a range within it
        let font = unsafe { ui.for_string(&s, range) };
        let Some(path) = path_of(&font) else { continue };
        if seen.contains(&path) {
            continue;
        }
        seen.push(path.clone());
        if let Some((bytes, index)) = face(&font) {
            lightcraft_ui_egui::theme::add_script_fallback_font(&format!("system {script}"), bytes, index, script);
        }
    }
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
