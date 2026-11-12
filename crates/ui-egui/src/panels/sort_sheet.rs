//! The phone layout's Sort sheet: what the grid is sorted by (rows with a check mark), the order,
//! and how photos are grouped by date, all finger-sized, each applied as it's tapped. The desktop's
//! Sort menu ([`super::bottombar::sort_menu`]) as a bottom sheet; opened by Sort on the album row.

use egui::{Stroke, vec2};
use lightcraft_catalog::{GroupBy, SortKey};
use serde_json::json;

use super::filter_sheet::{caption, head, pill, pills, value_row, wide_button};
use crate::LightcraftApp;
use crate::theme::Tokens;
use crate::widgets::register;

const KEYS: &[(&str, SortKey, &str)] = &[
    ("Capture Date", SortKey::CaptureDate, "captureDate"),
    ("Import Date", SortKey::ImportDate, "importDate"),
    ("Modified Date", SortKey::EditDate, "editDate"),
    ("File Name", SortKey::FileName, "fileName"),
    ("Rating", SortKey::Rating, "rating"),
    ("File Size", SortKey::FileSize, "fileSize"),
    ("Random", SortKey::Random, "random"),
];

const GROUPS: &[(&str, GroupBy, &str)] = &[
    ("Automatic", GroupBy::Auto, "auto"),
    ("Day", GroupBy::Day, "day"),
    ("Month", GroupBy::Month, "month"),
    ("Year", GroupBy::Year, "year"),
    ("None", GroupBy::None, "none"),
];

fn sheet_id() -> egui::Id {
    egui::Id::unique("compact-sort-sheet")
}

pub fn open(ctx: &egui::Context) {
    ctx.data_mut(|d| d.insert_temp(sheet_id(), true));
}

pub fn is_open(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(sheet_id())).unwrap_or(false)
}

pub(crate) fn close(ctx: &egui::Context) {
    ctx.data_mut(|d| d.remove::<bool>(sheet_id()));
}

/// Draw the sheet when it's open: at the bottom of the screen over a dimmed backdrop.
pub fn show(app: &mut LightcraftApp, ctx: &egui::Context) {
    if !is_open(ctx) {
        return;
    }
    let t = Tokens::get(ctx);
    let screen = ctx.content_rect();
    let width = screen.width().min(560.0);
    let area = egui::Area::new(egui::Id::unique("compact-sort-area"))
        .anchor(egui::Align2::CENTER_BOTTOM, vec2(0.0, 0.0))
        .default_width(width)
        .order(egui::Order::Foreground);
    let frame = egui::Frame::NONE
        .fill(t.chrome)
        .stroke(Stroke::new(1.0, t.divider))
        .corner_radius(egui::CornerRadius { nw: 14, ne: 14, sw: 0, se: 0 })
        .inner_margin(egui::Margin { left: 16, right: 16, top: 6, bottom: 16 });
    let body_max = (screen.height() * 0.75 - 140.0).max(160.0);
    let mut done = false;
    let modal = egui::Modal::new(egui::Id::unique("compact-sort-modal")).area(area).frame(frame).show(ctx, |ui| {
        ui.set_width(width - 32.0);
        register(ui, "panel:sortSheet", ui.max_rect());
        done = page(app, ui, body_max);
    });
    if done || modal.should_close() {
        close(ctx);
    }
}

/// The sheet's controls; true when it should close.
fn page(app: &mut LightcraftApp, ui: &mut egui::Ui, body_max: f32) -> bool {
    let cur = app.session.sort;
    let closed = head(ui, false, crate::i18n::tr("Sort"), |_| {});
    egui::ScrollArea::vertical().id_salt("sort-sheet").max_height(body_max).auto_shrink([false, true]).show(ui, |ui| {
        caption(ui, "Sort by");
        for (label, key, k) in KEYS {
            if value_row(ui, format!("sheetSort:key-{k}"), crate::i18n::tr(label), cur.key == *key).clicked() {
                let _ = app.run("library.sort", json!({"key": k}));
            }
        }
        if cur.key == SortKey::Random {
            ui.add_space(6.0);
            pills(ui, |ui| {
                if pill(ui, "sheetSort:reshuffle", None, "Reshuffle", false) {
                    let _ = app.run("library.shuffle", json!({}));
                }
            });
        }
        // a view that holds its photos shows what matches now when asked to
        if app.session.hold_view {
            ui.add_space(6.0);
            pills(ui, |ui| {
                if pill(ui, "button:refreshView", None, "Refresh View", false) {
                    let _ = app.run("library.refresh", json!({}));
                }
            });
        }
        // a shuffle has no direction worth choosing
        if cur.key != SortKey::Random {
            caption(ui, "Order");
            pills(ui, |ui| {
                for (label, asc, k) in [("Ascending", true, "asc"), ("Descending", false, "desc")] {
                    if pill(ui, &format!("sheetSort:{k}"), None, label, cur.ascending == asc) {
                        let _ = app.run("library.sort", json!({"ascending": asc}));
                    }
                }
            });
        }
        caption(ui, "Group by date");
        pills(ui, |ui| {
            for (label, g, k) in GROUPS {
                if pill(ui, &format!("sheetSort:group-{k}"), None, label, cur.group == *g) {
                    let _ = app.run("library.sort", json!({"group": k}));
                }
            }
        });
    });
    ui.add_space(12.0);
    closed | wide_button(ui, "sortSheetDone", crate::i18n::tr("Done"), true, true)
}
