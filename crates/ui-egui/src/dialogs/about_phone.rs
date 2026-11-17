//! About on an upright phone, as its Settings: a sheet with a header (‹, the page's name, ✕) and
//! pages pushed from rows: Contributors, Models, What's New and System Info open inside the sheet
//! and ‹ comes back; Send Feedback, Help and the links open in the browser.

use serde_json::json;

use crate::LightcraftApp;
use crate::theme::Tokens;
use crate::widgets::register;

/// The pages, in the order of the first page's rows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Page {
    #[default]
    Main,
    Contributors,
    Models,
    WhatsNew,
    SystemInfo,
}

impl Page {
    fn title(self) -> &'static str {
        match self {
            Page::Main => "About LightCraft",
            Page::Contributors => "Contributors",
            Page::Models => "Models",
            Page::WhatsNew => "What's New",
            Page::SystemInfo => "System Info",
        }
    }
}

const ROW_H: f32 = 48.0;

/// A full-width row: `label` at the left, `value` dimmed at the right, a › when it opens a page.
pub(crate) fn row(ui: &mut egui::Ui, id: &str, label: &str, value: Option<&str>, page: bool) -> bool {
    let t = Tokens::get(ui.ctx());
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(egui::vec2(w, ROW_H), egui::Sense::click());
    register(ui, id, r);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
    let p = ui.painter();
    if resp.is_pointer_button_down_on() {
        p.rect_filled(r, 8.0, t.hover);
    }
    let mut right = r.right() - 8.0;
    if page {
        let chevron = egui::Rect::from_center_size(egui::pos2(r.right() - 12.0, r.center().y), egui::vec2(16.0, 16.0));
        crate::icons::paint(p, chevron, crate::icons::Icon::ChevronRight, t.text_dim);
        right -= 22.0;
    }
    if let Some(v) = value {
        p.text(egui::pos2(right, r.center().y), egui::Align2::RIGHT_CENTER, v, t.font(15.0), t.text_dim);
    }
    p.text(egui::pos2(r.left() + 8.0, r.center().y), egui::Align2::LEFT_CENTER, label, t.font(16.0), t.text);
    resp.clicked()
}

/// A two-line row: `title` over `detail` (dimmed), a › at the right when `page`.
pub(crate) fn two_line_row(ui: &mut egui::Ui, id: &str, title: &str, detail: &str, page: bool) -> bool {
    let t = Tokens::get(ui.ctx());
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(egui::vec2(w, 60.0), egui::Sense::click());
    register(ui, id, r);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, title));
    let p = ui.painter();
    if resp.is_pointer_button_down_on() {
        p.rect_filled(r, 8.0, t.hover);
    }
    if page {
        let chevron = egui::Rect::from_center_size(egui::pos2(r.right() - 12.0, r.center().y), egui::vec2(16.0, 16.0));
        crate::icons::paint(p, chevron, crate::icons::Icon::ChevronRight, t.text_dim);
    }
    p.text(egui::pos2(r.left() + 8.0, r.center().y - 10.0), egui::Align2::LEFT_CENTER, title, t.font(16.0), t.text);
    p.text(egui::pos2(r.left() + 8.0, r.center().y + 11.0), egui::Align2::LEFT_CENTER, detail, t.font(13.0), t.text_dim);
    resp.clicked()
}

/// A phone's segmented control: `options` side by side in a rounded track, `selected` filled.
pub(crate) fn segmented(ui: &mut egui::Ui, id: &str, options: &[&str], selected: &mut usize) -> bool {
    let t = Tokens::get(ui.ctx());
    let w = ui.available_width();
    let (track, _) = ui.allocate_exact_size(egui::vec2(w, 36.0), egui::Sense::hover());
    ui.painter().rect_filled(track, 9.0, t.inset);
    let n = options.len().max(1);
    let seg = track.width() / n as f32;
    let mut changed = false;
    for (i, label) in options.iter().enumerate() {
        let r = egui::Rect::from_min_size(egui::pos2(track.left() + seg * i as f32, track.top()), egui::vec2(seg, track.height()));
        let wid = format!("{id}-{i}");
        let resp = ui.interact(r, egui::Id::unique(&wid), egui::Sense::click());
        register(ui, wid, r);
        let label = crate::i18n::tr(label);
        crate::widgets::announce_selected(&resp, label, *selected == i);
        if *selected == i {
            ui.painter().rect_filled(r.shrink(2.0), 7.0, t.button);
        }
        ui.painter().text(r.center(), egui::Align2::CENTER_CENTER, label, t.font(14.0), if *selected == i { t.text } else { t.text_dim });
        if resp.clicked() && *selected != i {
            *selected = i;
            changed = true;
        }
    }
    changed
}

/// The system's Back in About: a page goes back to the first. False: on the first already.
pub(crate) fn back(ctx: &egui::Context) -> bool {
    let key = egui::Id::unique("about-phone-page");
    match ctx.data(|d| d.get_temp::<(Page, u64)>(key)) {
        Some((page, pass)) if page != Page::Main => {
            ctx.data_mut(|d| d.insert_temp(key, (Page::Main, pass)));
            true
        }
        _ => false,
    }
}

/// About's sheet: the header, then the page under it, which scrolls on its own (`body_max`: the
/// room the sheet has for both).
pub(super) fn show(app: &mut LightcraftApp, ui: &mut egui::Ui, t: &Tokens, body_max: f32) {
    ui.set_width(ui.available_width());
    let top = ui.cursor().top();
    let key = egui::Id::unique("about-phone-page");
    let pass = ui.ctx().cumulative_pass_nr();
    let (mut page, last) = ui.data(|d| d.get_temp::<(Page, u64)>(key)).unwrap_or_default();
    // (shown again: it starts on its first page)
    if last + 1 < pass {
        page = Page::Main;
    }
    let back = (page != Page::Main).then_some("button:aboutBack");
    if crate::panels::settings::sheet_header(ui, crate::i18n::tr(page.title()), back, "button:aboutClose") {
        page = Page::Main;
    }
    ui.separator();
    let room = (body_max - (ui.cursor().top() - top)).max(120.0);
    let shown = page;
    egui::ScrollArea::vertical().id_salt(("about-phone", shown as u8)).max_height(room).auto_shrink([false, true]).show(ui, |ui| match shown {
        Page::Main => main_page(app, ui, t, &mut page),
        Page::Contributors => super::credits::phone_contributors(app, ui),
        Page::Models => super::credits::phone_models(ui),
        Page::WhatsNew => whats_new(ui, t),
        Page::SystemInfo => system_info(app, ui, t),
    });
    ui.data_mut(|d| d.insert_temp(key, (page, pass)));
}

fn main_page(app: &mut LightcraftApp, ui: &mut egui::Ui, t: &Tokens, page: &mut Page) {
    ui.add_space(8.0);
    ui.vertical_centered(|ui| {
        ui.label(egui::RichText::new("LightCraft").font(t.semibold(22.0)).color(t.text));
        ui.label(egui::RichText::new(crate::i18n::tr_format!("Version {}", env!("CARGO_PKG_VERSION"))).font(t.font(15.0)).color(t.text_dim));
        ui.label(
            egui::RichText::new(crate::i18n::tr("A clean-room, pure-Rust photo library and raw developer.")).font(t.font(14.0)).color(t.text_dim),
        );
    });
    ui.add_space(8.0);
    for (id, to) in [("about:contributors", Page::Contributors), ("about:models", Page::Models)] {
        if row(ui, id, crate::i18n::tr(to.title()), None, true) {
            *page = to;
        }
    }
    ui.separator();
    // (the Help menu's rows: What's New and System Info are pages here, so ‹ comes back to About)
    if row(ui, "about:app.whatsNew", crate::i18n::tr("What's New"), None, true) {
        *page = Page::WhatsNew;
    }
    for (id, label) in [("app.feedback", "Send Feedback…"), ("app.help", "LightCraft Help")] {
        if row(ui, &format!("about:{id}"), crate::i18n::tr(label), None, false)
            && let Err(e) = crate::menubar::run_item(app, id, json!({}))
        {
            app.toast(ui.ctx(), e);
        }
    }
    if row(ui, "about:app.systemInfo", crate::i18n::tr("System Info"), None, true) {
        *page = Page::SystemInfo;
    }
    ui.separator();
    for (id, label, url) in [
        ("about:website", "LightCraft website", crate::links::APP_PAGE),
        ("about:github", "Source code on GitHub", crate::links::GITHUB),
        ("about:artcraft", "ArtCraft — more creative apps", crate::links::WEBSITE),
    ] {
        if row(ui, id, crate::i18n::tr(label), None, false) {
            let _ = crate::links::open(app, url);
        }
    }
    ui.add_space(12.0);
    let w = ui.available_width();
    let r = crate::button::Button::new("Join the ArtCraft Discord").primary().large().min_width(w).widget("button:aboutDiscord").show(ui);
    if r.clicked() {
        let _ = crate::links::open(app, crate::links::DISCORD);
    }
    ui.add_space(8.0);
    ui.vertical_centered(|ui| {
        let credits =
            crate::i18n::tr_format!("MIT OR Apache-2.0. Fonts: {} (OFL). Icons: original.", crate::theme::font_credits(app.chinese_font.is_some()));
        ui.label(egui::RichText::new(credits).font(t.font(13.0)).color(t.text_dim));
    });
    ui.add_space(8.0);
}

/// The release notes in a phone's sizes: headings, then paragraphs and bullets that wrap at the
/// screen's width (the file's own line breaks inside one are joined).
fn whats_new(ui: &mut egui::Ui, t: &Tokens) {
    ui.spacing_mut().item_spacing.y = 6.0;
    for block in release_blocks(super::WHATS_NEW) {
        match block {
            Block::Date(h) => {
                ui.add_space(10.0);
                ui.label(egui::RichText::new(crate::i18n::tr(h)).font(t.semibold(18.0)).color(t.text));
            }
            Block::Heading(h) => {
                ui.add_space(6.0);
                ui.label(egui::RichText::new(crate::i18n::tr(h)).font(t.semibold(15.0)).color(t.text));
            }
            Block::Bullet(b) => {
                ui.label(egui::RichText::new(format!("•  {}", b.replace('`', ""))).font(t.font(15.0)).color(t.text_label));
            }
            Block::Text(b) => {
                ui.label(egui::RichText::new(b.replace('`', "")).font(t.font(15.0)).color(t.text_label));
            }
        }
    }
    ui.add_space(8.0);
}

#[derive(Debug, PartialEq)]
enum Block<'a> {
    Date(&'a str),
    Heading(&'a str),
    Bullet(String),
    Text(String),
}

/// The Markdown of the release notes as blocks: `##` and `###` headings, then bullets and
/// paragraphs with their wrapped lines joined.
fn release_blocks(md: &str) -> Vec<Block<'_>> {
    let mut out = Vec::new();
    for line in md.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with("# ") {
            // (a blank line ends a paragraph: the next text starts a new one)
            out.push(Block::Text(String::new()));
        } else if let Some(h) = l.strip_prefix("### ") {
            out.push(Block::Heading(h));
        } else if let Some(h) = l.strip_prefix("## ") {
            out.push(Block::Date(h));
        } else if let Some(b) = l.strip_prefix("- ") {
            out.push(Block::Bullet(b.to_string()));
        } else {
            match out.last_mut() {
                Some(Block::Bullet(s) | Block::Text(s)) => {
                    if !s.is_empty() {
                        s.push(' ');
                    }
                    s.push_str(l);
                }
                _ => out.push(Block::Text(l.to_string())),
            }
        }
    }
    out.retain(|b| !matches!(b, Block::Text(s) if s.is_empty()));
    out
}

/// What a phone's owner can use: the version, the device, the GPU, the library's size, and a
/// button that copies everything (frame times, memory budget …) for a bug report.
const PHONE_ROWS: [&str; 5] = ["Version", "System", "GPU", "Photos", "Albums"];

fn system_info(app: &mut LightcraftApp, ui: &mut egui::Ui, t: &Tokens) {
    let rows = match crate::menubar::run_item(app, "app.systemInfo", json!({"open": false})) {
        Ok(v) => v,
        Err(e) => {
            ui.label(e);
            return;
        }
    };
    let rows: Vec<(String, String)> = rows
        .as_array()
        .into_iter()
        .flatten()
        .map(|r| (r["label"].as_str().unwrap_or("").to_string(), r["value"].as_str().unwrap_or("").to_string()))
        .collect();
    ui.add_space(4.0);
    for (k, v) in rows.iter().filter(|(k, _)| PHONE_ROWS.contains(&k.as_str())) {
        let v = if k == "System" { system_name(v) } else { v.clone() };
        let id = format!("sysinfo:{k}");
        // (read-only: a row without a page)
        row(ui, &id, crate::i18n::tr(k), Some(&v), false);
    }
    ui.add_space(12.0);
    let w = ui.available_width();
    let r = crate::button::Button::new("Copy Details").large().min_width(w).widget("button:sysinfoCopy").show(ui);
    if r.clicked() {
        let text: String = rows.iter().map(|(k, v)| format!("{k}: {v}\n")).collect();
        ui.ctx().copy_text(text);
        app.toast(ui.ctx(), crate::i18n::tr("Copied"));
    }
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new(crate::i18n::tr("Copies these and the app's performance figures, for a bug report or feedback."))
            .font(t.font(13.0))
            .color(t.text_dim),
    );
}

/// "ios (aarch64)" as a person reads it: "iOS · arm64".
fn system_name(raw: &str) -> String {
    let (os, arch) = raw.split_once(" (").map_or((raw, ""), |(o, a)| (o, a.trim_end_matches(')')));
    let os = match os {
        "ios" => "iOS",
        "android" => "Android",
        "macos" => "macOS",
        "linux" => "Linux",
        "windows" => "Windows",
        other => other,
    };
    let arch = match arch {
        "aarch64" => "arm64",
        other => other,
    };
    if arch.is_empty() { os.to_string() } else { format!("{os} · {arch}") }
}

#[cfg(test)]
mod tests {
    use super::Block;

    #[test]
    fn release_notes_join_their_wrapped_lines() {
        let md = "# What's new\n\n## 2026-10-30\n\n### A title\n\n- one\n  two\n- three\n\nsome\ntext\n";
        assert_eq!(
            super::release_blocks(md),
            [
                Block::Date("2026-10-30"),
                Block::Heading("A title"),
                Block::Bullet("one two".into()),
                Block::Bullet("three".into()),
                Block::Text("some text".into())
            ]
        );
    }

    #[test]
    fn system_names_read_as_people_say_them() {
        assert_eq!(super::system_name("ios (aarch64)"), "iOS · arm64");
        assert_eq!(super::system_name("android (aarch64)"), "Android · arm64");
        assert_eq!(super::system_name("plan9"), "plan9");
    }
}
