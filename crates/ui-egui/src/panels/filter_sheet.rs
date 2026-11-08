//! The phone layout's Filter sheet: the desktop filter bar's controls ([`super::filterbar`]) as a
//! bottom sheet with finger-sized targets. Rating (stars and at least / exactly / at most), flags
//! and colour labels (several at once), type and edited state, then Camera, Lens, Keyword and
//! filter presets, each a list of its own. Every control drives `library.filter` as it's tapped,
//! so the grid behind the sheet (and its chips) follows; "Show N photos" closes the sheet.
//! Opened by the Filter button of [`super::compact`]'s library bar.

use egui::{Color32, Rect, Sense, Stroke, StrokeKind, pos2, vec2};
use lightcraft_catalog::{ColorLabel, Flag, MediaKind, RatingOp};
use serde_json::{Value, json};

use crate::LightcraftApp;
use crate::icons::{Icon, paint};
use crate::theme::{Tokens, label_color};
use crate::widgets::{icon_button, register};

/// Which page of the sheet shows.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Page {
    #[default]
    Main,
    /// The values of `camera`, `lens` or `keyword`.
    Values(&'static str),
    Presets,
}

fn sheet_id() -> egui::Id {
    egui::Id::unique("compact-filter-sheet")
}

pub fn open(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(sheet_id(), Page::Main));
}

pub fn is_open(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<Page>(sheet_id()).is_some())
}

fn close(ctx: &egui::Context) {
    ctx.data_mut(|d| d.remove_temp::<Page>(sheet_id()));
}

fn filter(app: &mut LightcraftApp, patch: Value) {
    let _ = app.run("library.filter", patch);
}

/// How many filters are on (each chip above the grid but the search's, which shows in its field):
/// the Filter button's badge.
pub fn active_count(app: &LightcraftApp) -> usize {
    lightcraft_engine::filter_chips(&app.session.filter, &app.session.catalog).iter().filter(|c| c.clear.get("text").is_none()).count()
}

/// Draw the sheet when it's open: at the bottom of the screen over a dimmed backdrop.
pub fn show(app: &mut LightcraftApp, ctx: &egui::Context) {
    let Some(mut page) = ctx.data(|d| d.get_temp::<Page>(sheet_id())) else { return };
    let t = Tokens::get(ctx);
    let screen = ctx.content_rect();
    let width = screen.width().min(560.0);
    let area = egui::Area::new(egui::Id::unique("compact-filter-area"))
        .anchor(egui::Align2::CENTER_BOTTOM, vec2(0.0, 0.0))
        .default_width(width)
        .order(egui::Order::Foreground);
    let frame = egui::Frame::NONE
        .fill(t.chrome)
        .stroke(Stroke::new(1.0, t.divider))
        .corner_radius(egui::CornerRadius { nw: 14, ne: 14, sw: 0, se: 0 })
        .inner_margin(egui::Margin { left: 16, right: 16, top: 6, bottom: 16 });
    // the sheet leaves the top of the screen (the bar, the first photos) in sight
    let body_max = (screen.height() * 0.78 - 140.0).max(160.0);
    let mut closing = false;
    let modal = egui::Modal::new(egui::Id::unique("compact-filter-modal")).area(area).frame(frame).show(ctx, |ui| {
        ui.set_width(width - 32.0);
        register(ui, "panel:filterSheet", ui.max_rect());
        match page {
            Page::Main => closing = main_page(app, ui, &mut page, body_max),
            Page::Values(key) => values_page(app, ui, &mut page, key, body_max),
            Page::Presets => presets_page(app, ui, &mut page, &mut closing, body_max),
        }
    });
    if closing || (modal.should_close() && page == Page::Main) {
        close(ctx);
        return;
    }
    if modal.should_close() {
        page = Page::Main;
    }
    ctx.data_mut(|d| d.insert_temp(sheet_id(), page));
}

/// The sheet's head: ✕ or ‹ at the left, the title, and `right` at the right. True when ✕/‹ is tapped.
fn head(ui: &mut egui::Ui, back: bool, title: &str, right: impl FnOnce(&mut egui::Ui)) -> bool {
    let t = Tokens::get(ui.ctx());
    let mut left = false;
    ui.horizontal(|ui| {
        ui.set_height(44.0);
        let (icon, tip) = if back { (Icon::ChevronLeft, "Back") } else { (Icon::Close, "Close") };
        left = icon_button(ui, "filterSheetClose", icon, vec2(40.0, 40.0), false, true, tip).clicked();
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            right(ui);
            ui.centered_and_justified(|ui| ui.label(egui::RichText::new(crate::i18n::tr(title)).font(t.semibold(16.0)).color(t.text)));
        });
    });
    ui.add_space(4.0);
    left
}

/// A section's caption.
fn caption(ui: &mut egui::Ui, text: &str) {
    let t = Tokens::get(ui.ctx());
    ui.add_space(10.0);
    ui.label(egui::RichText::new(crate::i18n::tr(text)).font(t.semibold(13.0)).color(t.text_dim));
    ui.add_space(4.0);
}

/// A rounded toggle with a label (and an optional icon before it); true when tapped.
fn pill(ui: &mut egui::Ui, id: &str, icon: Option<(Icon, Color32)>, label: &str, on: bool) -> bool {
    let t = Tokens::get(ui.ctx());
    let label = crate::i18n::tr(label);
    let font = t.font(14.0);
    let text_w = ui.painter().layout_no_wrap(label.to_string(), font.clone(), t.text).size().x;
    let icon_w = if icon.is_some() { 24.0 } else { 0.0 };
    let (r, resp) = ui.allocate_exact_size(vec2(text_w + icon_w + 28.0, 36.0), Sense::click());
    crate::widgets::announce_selected(&resp, label, on);
    register(ui, format!("sheetFilter:{id}"), r);
    let p = ui.painter();
    let fill = if on {
        t.tool_active
    } else if resp.is_pointer_button_down_on() {
        t.hover
    } else {
        t.field
    };
    p.rect(r, 18.0, fill, Stroke::new(1.0, if on { t.accent } else { t.field_border }), StrokeKind::Inside);
    let mut x = r.left() + 14.0;
    if let Some((icon, color)) = icon {
        paint(p, Rect::from_center_size(pos2(x + 9.0, r.center().y), vec2(18.0, 18.0)), icon, color);
        x += icon_w;
    }
    p.text(pos2(x, r.center().y), egui::Align2::LEFT_CENTER, label, font, if on { t.text } else { t.text_label });
    resp.clicked()
}

/// Pills that wrap onto as many rows as they need.
fn pills(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    ui.horizontal_wrapped(|ui| {
        ui.spacing_mut().item_spacing = vec2(8.0, 8.0);
        add(ui);
    });
}

/// A row that opens a list: the label, the current value and ›. True when tapped.
fn nav_row(ui: &mut egui::Ui, id: &str, label: &str, value: &str, active: bool) -> bool {
    let t = Tokens::get(ui.ctx());
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(vec2(w, 48.0), Sense::click());
    register(ui, format!("sheetFilter:{id}"), r);
    let p = ui.painter();
    if resp.is_pointer_button_down_on() {
        p.rect_filled(r, 6.0, t.hover);
    }
    p.line_segment([r.left_top(), r.right_top()], Stroke::new(1.0, t.divider));
    p.text(pos2(r.left() + 4.0, r.center().y), egui::Align2::LEFT_CENTER, crate::i18n::tr(label), t.font(16.0), t.text);
    paint(p, Rect::from_center_size(pos2(r.right() - 12.0, r.center().y), vec2(16.0, 16.0)), Icon::ChevronRight, t.text_dim);
    let room = (w * 0.55).max(40.0);
    let font = t.font(15.0);
    let shown = crate::widgets::elide_head(value, room, |s| ui.painter().layout_no_wrap(s.to_string(), font.clone(), t.text).size().x);
    p.text(pos2(r.right() - 28.0, r.center().y), egui::Align2::RIGHT_CENTER, shown, font, if active { t.accent } else { t.text_dim });
    resp.clicked()
}

/// A full-width button; `primary` is filled with the accent. True when tapped.
fn wide_button(ui: &mut egui::Ui, id: &str, label: &str, primary: bool, enabled: bool) -> bool {
    let t = Tokens::get(ui.ctx());
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(vec2(w, 46.0), if enabled { Sense::click() } else { Sense::hover() });
    register(ui, format!("button:{id}"), r);
    let p = ui.painter();
    let (fill, text) = match (primary, enabled) {
        (true, true) => (t.accent, Color32::WHITE),
        (true, false) => (t.field, t.text_dim),
        (false, e) => (t.field, if e { t.text } else { t.text_dim }),
    };
    let fill = if enabled && resp.is_pointer_button_down_on() { fill.gamma_multiply(0.8) } else { fill };
    p.rect(r, 10.0, fill, Stroke::new(1.0, if primary { Color32::TRANSPARENT } else { t.field_border }), StrokeKind::Inside);
    p.text(r.center(), egui::Align2::CENTER_CENTER, label, t.semibold(15.0), text);
    enabled && resp.clicked()
}

/// The filters; true when the sheet should close.
fn main_page(app: &mut LightcraftApp, ui: &mut egui::Ui, page: &mut Page, body_max: f32) -> bool {
    let t = Tokens::get(ui.ctx());
    let filtering = active_count(app) > 0;
    let mut reset = false;
    let closed = head(ui, false, "Filter", |ui| {
        let r = ui.add_enabled(filtering, egui::Button::new(egui::RichText::new(crate::i18n::tr("Reset")).font(t.font(15.0))).frame(false));
        register(ui, "button:filterSheetReset", r.rect);
        reset = r.clicked();
    });
    if reset {
        // the search stays: it has its own field (and ✕) in the bar
        let text = app.session.filter.text.clone();
        let _ = app.run("library.clearFilter", json!({}));
        if !text.is_empty() {
            filter(app, json!({"text": text}));
        }
    }
    let f = app.session.filter.clone();
    egui::ScrollArea::vertical().max_height(body_max).auto_shrink([false, true]).show(ui, |ui| {
        caption(ui, "Rating");
        stars(app, ui, f.rating);
        if f.rating > 0 {
            ui.add_space(6.0);
            pills(ui, |ui| {
                for (id, label, op) in
                    [("atLeast", "At least", RatingOp::AtLeast), ("exactly", "Exactly", RatingOp::Exactly), ("atMost", "At most", RatingOp::AtMost)]
                {
                    if pill(ui, &format!("ratingOp-{id}"), None, label, f.rating_op == op) {
                        filter(app, json!({"ratingOp": id}));
                    }
                }
            });
        }

        caption(ui, "Flag");
        let chosen: Vec<Flag> = f.flags_shown().unwrap_or_default();
        pills(ui, |ui| {
            for (id, icon, flag, label) in [
                ("pick", Icon::FlagPickFilled, Flag::Pick, "Picked"),
                ("reject", Icon::FlagReject, Flag::Reject, "Rejected"),
                ("none", Icon::Circle, Flag::None, "Unflagged"),
            ] {
                let on = chosen.contains(&flag);
                let color = match flag {
                    Flag::Pick => t.pick,
                    Flag::Reject => t.reject,
                    Flag::None => t.icon,
                };
                if pill(ui, &format!("flag-{id}"), Some((icon, color)), label, on) {
                    // in the bar's order, as the desktop bar sends them
                    let names: Vec<String> = [Flag::Pick, Flag::Reject, Flag::None]
                        .into_iter()
                        .filter(|x| if *x == flag { !on } else { chosen.contains(x) })
                        .map(|x| format!("{x:?}").to_lowercase())
                        .collect();
                    filter(app, json!({"flag": Value::Null, "flags": names}));
                }
            }
        });

        caption(ui, "Color label");
        labels(app, ui, &f.labels_shown().unwrap_or_default());

        caption(ui, "Type");
        pills(ui, |ui| {
            let none = f.kind.is_none() && f.merged.is_none();
            for (id, label, kind) in [
                ("all", "All", None),
                ("image", "Photos", Some(MediaKind::Image)),
                ("raw", "Raw", Some(MediaKind::Raw)),
                ("video", "Videos", Some(MediaKind::Video)),
            ] {
                let on = if kind.is_none() { none } else { f.kind == kind && f.merged.is_none() };
                if pill(ui, &format!("kind-{id}"), None, label, on) {
                    filter(app, json!({"kind": kind.map(|_| id), "merged": null}));
                }
            }
            for (id, label) in [("hdr", "HDR"), ("panorama", "Panoramas"), ("hdrPanorama", "HDR Panoramas")] {
                if pill(ui, &format!("kind-{id}"), None, label, f.merged.as_deref() == Some(id)) {
                    filter(app, json!({"kind": null, "merged": id}));
                }
            }
        });

        caption(ui, "Edits");
        pills(ui, |ui| {
            for (id, label, e) in [("any", "Any", None), ("edited", "Edited", Some(true)), ("unedited", "Unedited", Some(false))] {
                if pill(ui, &format!("edited-{id}"), None, label, f.edited == e) {
                    filter(app, json!({"edited": e}));
                }
            }
        });

        ui.add_space(12.0);
        for (key, label, any, current) in [
            ("camera", "Camera", "Any camera", f.camera.clone()),
            ("lens", "Lens", "Any lens", f.lens.clone()),
            ("keyword", "Keyword", "Any keyword", f.keyword.clone()),
        ] {
            let shown = current.clone().unwrap_or_else(|| crate::i18n::tr(any).to_string());
            if nav_row(ui, key, label, &shown, current.is_some()) {
                *page = Page::Values(key);
            }
        }
        if nav_row(ui, "presets", "Filter presets", &app.session.filter_presets.len().to_string(), false) {
            *page = Page::Presets;
        }
    });
    ui.add_space(12.0);
    let mut done = closed;
    if wide_button(ui, "filterSheetSave", crate::i18n::tr("Save as Smart Album…"), false, filtering) {
        let _ = app.run("dialog.newSmartAlbum", json!({}));
        done = true;
    }
    ui.add_space(8.0);
    let n = app.session.visible().len();
    let show =
        if n == 1 { crate::i18n::tr("Show 1 photo").to_string() } else { format!("{} {n} {}", crate::i18n::tr("Show"), crate::i18n::tr("photos")) };
    done |= wide_button(ui, "filterSheetDone", &show, true, true);
    done
}

/// Five big stars: tap one for "that many stars", tap the lit last one again for any rating.
fn stars(app: &mut LightcraftApp, ui: &mut egui::Ui, rating: u8) {
    let t = Tokens::get(ui.ctx());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for n in 1..=5u8 {
            let (r, resp) = ui.allocate_exact_size(vec2(44.0, 44.0), Sense::click());
            register(ui, format!("sheetFilter:rating-{n}"), r);
            crate::widgets::announce_selected(&resp, format!("{n} ★"), n == rating);
            let lit = n <= rating;
            paint(ui.painter(), r.shrink(9.0), if lit { Icon::StarFilled } else { Icon::Star }, if lit { t.text } else { t.icon });
            if resp.clicked() {
                filter(app, json!({"rating": if n == rating { 0 } else { n }}));
            }
        }
    });
}

/// The colour labels as big swatches; several can be on (any of them matches).
fn labels(app: &mut LightcraftApp, ui: &mut egui::Ui, chosen: &[ColorLabel]) {
    let t = Tokens::get(ui.ctx());
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        for l in ColorLabel::ALL {
            let on = chosen.contains(&l);
            let name = format!("{l:?}").to_lowercase();
            let (r, resp) = ui.allocate_exact_size(vec2(44.0, 44.0), Sense::click());
            register(ui, format!("sheetFilter:label-{name}"), r);
            crate::widgets::announce_selected(&resp, crate::i18n::color_label(&app.session.catalog, l), on);
            let p = ui.painter();
            p.circle_filled(r.center(), 14.0, label_color(l));
            if on {
                p.circle_stroke(r.center(), 19.0, Stroke::new(2.5, t.text));
            }
            if resp.clicked() {
                let mut next = chosen.to_vec();
                if on {
                    next.retain(|x| *x != l);
                } else {
                    next.push(l);
                }
                next.sort();
                let names: Vec<String> = next.iter().map(|x| format!("{x:?}").to_lowercase()).collect();
                filter(app, json!({"label": Value::Null, "labels": names}));
            }
        }
    });
}

/// A row of a list page: the label, ✓ when it's the current one. True when tapped.
fn value_row(ui: &mut egui::Ui, id: String, label: &str, on: bool) -> egui::Response {
    let t = Tokens::get(ui.ctx());
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(vec2(w, 46.0), Sense::click());
    register(ui, id, r);
    crate::widgets::announce_selected(&resp, label, on);
    let p = ui.painter();
    if resp.is_pointer_button_down_on() {
        p.rect_filled(r, 6.0, t.hover);
    }
    p.line_segment([r.left_top(), r.right_top()], Stroke::new(1.0, t.divider));
    let font = t.font(16.0);
    let room = w - 48.0;
    let shown = crate::widgets::elide_head(label, room, |s| ui.painter().layout_no_wrap(s.to_string(), font.clone(), t.text).size().x);
    p.text(pos2(r.left() + 4.0, r.center().y), egui::Align2::LEFT_CENTER, shown, font, if on { t.accent } else { t.text });
    if on {
        paint(p, Rect::from_center_size(pos2(r.right() - 14.0, r.center().y), vec2(18.0, 18.0)), Icon::Check, t.accent);
    }
    resp
}

/// The cameras, lenses or keywords in the library; tapping one filters by it and goes back.
fn values_page(app: &mut LightcraftApp, ui: &mut egui::Ui, page: &mut Page, key: &'static str, body_max: f32) {
    let (title, any) = match key {
        "camera" => ("Camera", "Any camera"),
        "lens" => ("Lens", "Any lens"),
        _ => ("Keyword", "Any keyword"),
    };
    if head(ui, true, title, |_| {}) {
        *page = Page::Main;
        return;
    }
    let f = &app.session.filter;
    let current = match key {
        "camera" => f.camera.clone(),
        "lens" => f.lens.clone(),
        _ => f.keyword.clone(),
    };
    let values = app.caches.filter_values(&app.session.catalog);
    let values = match key {
        "camera" => values.cameras.clone(),
        "lens" => values.lenses.clone(),
        _ => values.keywords.clone(),
    };
    let mut chosen = None;
    egui::ScrollArea::vertical().max_height(body_max).auto_shrink([false, true]).show(ui, |ui| {
        if value_row(ui, format!("sheetFilter:{key}-any"), crate::i18n::tr(any), current.is_none()).clicked() {
            chosen = Some(Value::Null);
        }
        for (i, v) in values.iter().enumerate() {
            if value_row(ui, format!("sheetFilter:{key}-{i}"), v, current.as_deref() == Some(v.as_str())).clicked() {
                chosen = Some(json!(v));
            }
        }
        if values.is_empty() {
            ui.add_space(8.0);
            ui.label(egui::RichText::new(crate::i18n::tr("None in this library yet")).color(Tokens::get(ui.ctx()).text_dim));
        }
    });
    if let Some(v) = chosen {
        filter(app, json!({key: v}));
        *page = Page::Main;
    }
}

/// The saved filter presets: tap one to apply it, the bin deletes it; and saving the current filter.
fn presets_page(app: &mut LightcraftApp, ui: &mut egui::Ui, page: &mut Page, closing: &mut bool, body_max: f32) {
    let t = Tokens::get(ui.ctx());
    if head(ui, true, "Filter presets", |_| {}) {
        *page = Page::Main;
        return;
    }
    let names: Vec<String> = app.session.filter_presets.iter().map(|f| f.name.clone()).collect();
    let mut apply = None;
    let mut delete = None;
    egui::ScrollArea::vertical().max_height(body_max).auto_shrink([false, true]).show(ui, |ui| {
        for (i, n) in names.iter().enumerate() {
            let row = value_row(ui, format!("sheetFilter:preset-{i}"), n, false);
            let bin = Rect::from_center_size(pos2(row.rect.right() - 20.0, row.rect.center().y), vec2(40.0, 40.0));
            let bin_resp = ui.interact(bin, ui.scope_id().with(("preset-delete", i)), Sense::click());
            register(ui, format!("sheetFilter:presetDelete-{i}"), bin);
            paint(ui.painter(), bin.shrink(11.0), Icon::Trash, t.text_dim);
            if bin_resp.clicked() {
                delete = Some(n.clone());
            } else if row.clicked() {
                apply = Some(n.clone());
            }
        }
        if names.is_empty() {
            ui.add_space(8.0);
            ui.label(egui::RichText::new(crate::i18n::tr("No filter presets yet")).color(t.text_dim));
        }
    });
    if let Some(n) = delete {
        let _ = app.run("filter.deletePreset", json!({"name": n}));
    }
    if let Some(n) = apply {
        let _ = app.run("filter.applyPreset", json!({"name": n}));
        *page = Page::Main;
    }
    ui.add_space(12.0);
    let filtering = active_count(app) > 0;
    if wide_button(ui, "filterSheetSavePreset", crate::i18n::tr("Save Current Filter as Preset…"), false, filtering) {
        crate::panels::dialogs::prompt(app, "Save Filter Preset", "Preset name", "", "filter.savePreset", json!({}), "name");
        *closing = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sheet_opens_and_closes() {
        let ctx = egui::Context::default();
        assert!(!is_open(&ctx));
        open(&ctx);
        assert!(is_open(&ctx));
        close(&ctx);
        assert!(!is_open(&ctx));
    }
}
