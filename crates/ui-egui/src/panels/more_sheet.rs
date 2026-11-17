//! The phone layout's "…" as a bottom sheet, for a host without a menu of its own (Android,
//! tests): the same list as the host's menu ([`crate::native_menu::more_items`]), drawn for a
//! finger. Sections show their title as a small header, rows have check marks, disabled and
//! destructive states, and a submenu opens as the next page with ‹ to go back.

use egui::{Rect, Sense, Stroke, vec2};
use serde_json::Value;

use crate::LightcraftApp;
use crate::icons::{Icon, paint};
use crate::menubar::MenuNode;
use crate::native_menu::NativeItem;
use crate::theme::Tokens;
use crate::widgets::{icon_button, register};

/// Height of a row: a finger's.
const ROW_H: f32 = 44.0;
/// Height of a section's title.
const HEADER_H: f32 = 26.0;
/// Height of the line between sections without a title.
const GAP_H: f32 = 9.0;

/// Where the sheet is: the editor's or the library's "…", and the submenus open from the top.
#[derive(Clone, Debug, PartialEq)]
struct State {
    editor: bool,
    path: Vec<String>,
}

fn state_id() -> egui::Id {
    egui::Id::unique("compact-more-sheet")
}

/// Open the sheet on its first page: the editor's "…" or the library's.
pub fn open(ctx: &egui::Context, editor: bool) {
    ctx.data_mut(|d| d.insert_temp(state_id(), State { editor, path: Vec::new() }));
}

pub fn is_open(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<State>(state_id()).is_some())
}

pub(crate) fn close(ctx: &egui::Context) {
    ctx.data_mut(|d| d.remove::<State>(state_id()));
}

/// Draw the sheet when it's open: at the bottom of the screen over a dimmed backdrop.
pub fn show(app: &mut LightcraftApp, ctx: &egui::Context) {
    let Some(mut st) = ctx.data(|d| d.get_temp::<State>(state_id())) else { return };
    let t = Tokens::get(ctx);
    let screen = ctx.content_rect();
    let width = screen.width().min(560.0);
    let area = egui::Area::new(egui::Id::unique("compact-more-area"))
        .anchor(egui::Align2::CENTER_BOTTOM, vec2(0.0, 0.0))
        .default_width(width)
        .order(egui::Order::Foreground);
    let frame = egui::Frame::NONE
        .fill(t.chrome)
        .stroke(Stroke::new(1.0, t.divider))
        .corner_radius(egui::CornerRadius { nw: 14, ne: 14, sw: 0, se: 0 })
        .inner_margin(egui::Margin { left: 12, right: 12, top: 6, bottom: 16 });
    let body_max = (screen.height() * 0.7 - 60.0).max(160.0);
    let items = crate::native_menu::more_items(app, st.editor);
    let mut run: Option<(String, Value)> = None;
    let mut closing = false;
    let modal = egui::Modal::new(egui::Id::unique("compact-more-modal")).area(area).frame(frame).show(ctx, |ui| {
        ui.set_width(width - 24.0);
        register(ui, "panel:moreSheet", ui.max_rect());
        page(ui, &items, &mut st, &mut run, &mut closing, body_max);
    });
    if modal.should_close() {
        // Escape or a tap outside goes back a page, then closes
        if st.path.pop().is_none() {
            closing = true;
        }
    }
    if closing || run.is_some() {
        close(ctx);
    } else {
        ctx.data_mut(|d| d.insert_temp(state_id(), st));
    }
    if let Some((id, params)) = run
        && let Err(e) = crate::menubar::run_item(app, &id, params)
    {
        app.toast(ctx, e);
    }
}

/// The items of the page at `path` (submenu labels from the top) and its title; a submenu that
/// has gone (the menu changes with the screen) gives its parent's page.
fn page_items<'a>(items: &'a [NativeItem], path: &mut Vec<String>) -> (&'a [NativeItem], Option<String>) {
    fn child<'a>(items: &'a [NativeItem], label: &str) -> Option<&'a [NativeItem]> {
        items.iter().find_map(|i| match i {
            NativeItem::Submenu { label: l, items, .. } if l == label => Some(items.as_slice()),
            NativeItem::Section { items, .. } => child(items, label),
            _ => None,
        })
    }
    let mut page = items;
    for (depth, label) in path.iter().enumerate() {
        match child(page, label) {
            Some(next) => page = next,
            None => {
                path.truncate(depth);
                break;
            }
        }
    }
    (page, path.last().cloned())
}

/// The height the rows of `items` take.
fn height(items: &[NativeItem]) -> f32 {
    items
        .iter()
        .map(|i| match i {
            NativeItem::Section { title, items } => (if title.is_empty() { GAP_H } else { HEADER_H }) + height(items),
            _ => ROW_H,
        })
        .sum()
}

fn page(ui: &mut egui::Ui, items: &[NativeItem], st: &mut State, run: &mut Option<(String, Value)>, closing: &mut bool, body_max: f32) {
    let (rows, title) = page_items(items, &mut st.path);
    let back = title.is_some();
    if head(ui, back, title.as_deref().unwrap_or_else(|| crate::i18n::tr("More"))) {
        if back {
            st.path.pop();
        } else {
            *closing = true;
        }
        return;
    }
    let mut open: Option<String> = None;
    // the page's own height (up to `body_max`), not what the last page left: the sheet's area
    // remembers its size, so a short page would otherwise squeeze the next one into a sliver
    let h = height(rows).min(body_max);
    // (a scroll state per page)
    egui::ScrollArea::vertical()
        .id_salt(("more-sheet", st.editor, &st.path))
        .max_height(body_max)
        .min_scrolled_height(h)
        .auto_shrink([false, true])
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            for (i, item) in rows.iter().enumerate() {
                match item {
                    NativeItem::Section { title, items } => {
                        if !title.is_empty() {
                            header(ui, title);
                        } else if i > 0 {
                            gap(ui);
                        }
                        for item in items {
                            item_row(ui, item, run, &mut open);
                        }
                    }
                    item => item_row(ui, item, run, &mut open),
                }
            }
        });
    if let Some(next) = open {
        st.path.push(next);
    }
}

/// One row: an action runs (`more:{id}`, its parameters after `|`), a submenu opens its page
/// (`submenu:{label}`).
fn item_row(ui: &mut egui::Ui, item: &NativeItem, run: &mut Option<(String, Value)>, open: &mut Option<String>) {
    match item {
        NativeItem::Action { id, params, label, enabled, checked, destructive, .. } => {
            let key = if params.as_object().is_some_and(|o| !o.is_empty()) { MenuNode::key(id, params) } else { id.clone() };
            if row(ui, format!("more:{key}"), label, *enabled, checked.then_some(true), false, *destructive) {
                *run = Some((id.clone(), params.clone()));
            }
        }
        NativeItem::Submenu { label, items, .. } => {
            if row(ui, format!("submenu:{label}"), label, !items.is_empty(), None, true, false) {
                *open = Some(label.clone());
            }
        }
        // (sections nest only at the top of a page)
        NativeItem::Section { items, .. } => {
            for item in items {
                item_row(ui, item, run, open);
            }
        }
    }
}

/// The sheet's head: ✕ or ‹ at the left and the title. True when ✕/‹ is tapped.
fn head(ui: &mut egui::Ui, back: bool, title: &str) -> bool {
    let t = Tokens::get(ui.ctx());
    let mut left = false;
    ui.horizontal(|ui| {
        ui.set_height(44.0);
        let (icon, tip) = if back { (Icon::ChevronLeft, "Back") } else { (Icon::Close, "Close") };
        left = icon_button(ui, "moreSheetClose", icon, vec2(40.0, 40.0), false, true, tip).clicked();
        ui.add_space(40.0);
        ui.centered_and_justified(|ui| ui.label(egui::RichText::new(title).font(t.semibold(16.0)).color(t.text)));
    });
    ui.add_space(2.0);
    left
}

/// A section's title: small and dim, over its rows.
fn header(ui: &mut egui::Ui, title: &str) {
    let t = Tokens::get(ui.ctx());
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), HEADER_H), Sense::hover());
    crate::widgets::register_geometry(ui.ctx(), format!("moreHeader:{title}"), r);
    ui.painter().text(egui::pos2(r.left() + 8.0, r.bottom() - 5.0), egui::Align2::LEFT_BOTTOM, title, t.semibold(13.0), t.text_dim);
}

/// A full-width row: a check mark gutter, the label (red when `destructive`), and › for a page;
/// true when tapped.
pub(crate) fn row(ui: &mut egui::Ui, id: String, label: &str, enabled: bool, checked: Option<bool>, page: bool, destructive: bool) -> bool {
    let t = Tokens::get(ui.ctx());
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(vec2(w, ROW_H), if enabled { Sense::click() } else { Sense::hover() });
    register(ui, id, r);
    crate::widgets::announce_selected(&resp, label, checked == Some(true));
    let p = ui.painter();
    if enabled && resp.is_pointer_button_down_on() {
        p.rect_filled(r, 8.0, t.hover);
    }
    let color = match (enabled, destructive) {
        (false, _) => t.text_disabled,
        (true, true) => t.reject,
        (true, false) => t.text,
    };
    if checked == Some(true) {
        paint(p, Rect::from_center_size(egui::pos2(r.left() + 14.0, r.center().y), vec2(16.0, 16.0)), Icon::Check, color);
    }
    let font = t.font(16.0);
    let room = w - 36.0 - if page { 28.0 } else { 8.0 };
    let shown = crate::widgets::elide_head(label, room, |s| ui.painter().layout_no_wrap(s.to_string(), font.clone(), color).size().x);
    p.text(egui::pos2(r.left() + 32.0, r.center().y), egui::Align2::LEFT_CENTER, shown, font, color);
    if page {
        paint(p, Rect::from_center_size(egui::pos2(r.right() - 14.0, r.center().y), vec2(16.0, 16.0)), Icon::ChevronRight, t.text_dim);
    }
    enabled && resp.clicked()
}

/// A thin line between groups of rows.
fn gap(ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), GAP_H), Sense::hover());
    ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, t.divider));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sheet_opens_and_closes() {
        let ctx = egui::Context::default();
        assert!(!is_open(&ctx));
        open(&ctx, false);
        assert!(is_open(&ctx));
        close(&ctx);
        assert!(!is_open(&ctx));
    }

    #[test]
    fn a_page_that_has_gone_falls_back_to_its_parent() {
        let leaf = NativeItem::Action {
            id: "a".into(),
            params: serde_json::json!({}),
            label: "A".into(),
            symbol: "",
            enabled: true,
            checked: false,
            destructive: false,
        };
        let items = vec![NativeItem::Section {
            title: "S".into(),
            items: vec![NativeItem::Submenu { label: "Sub".into(), symbol: "", items: vec![leaf.clone()] }],
        }];
        let mut path = vec!["Sub".to_string()];
        let (rows, title) = page_items(&items, &mut path);
        assert_eq!(rows, std::slice::from_ref(&leaf));
        assert_eq!(title.as_deref(), Some("Sub"));
        let mut path = vec!["Sub".to_string(), "Gone".to_string()];
        let (_, title) = page_items(&items, &mut path);
        assert_eq!(path, ["Sub"], "the missing page is dropped");
        assert_eq!(title.as_deref(), Some("Sub"));
        assert!((height(&items) - (HEADER_H + ROW_H)).abs() < 1e-3);
    }
}
