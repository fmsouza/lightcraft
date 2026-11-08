//! The phone layout's "…" as a bottom sheet: what the screen needs most (the editor's photo
//! settings, the library's import and selection), then **All Menus**, the desktop menu bar as
//! pages a finger can use: a menu's items in full-width rows, a submenu opens as the next page,
//! ‹ goes back. It draws [`crate::menubar::menu_bar`], so every desktop command is here with the
//! same enabled state and check marks; commands that get a phone screen of their own move out of
//! it into the first page (or a sheet of their own) over time.

use egui::{Rect, Sense, Stroke, vec2};
use serde_json::{Value, json};

use crate::LightcraftApp;
use crate::icons::{Icon, paint};
use crate::menubar::MenuNode;
use crate::theme::Tokens;
use crate::widgets::{icon_button, register};

/// Where the sheet is: the first page of `quick` actions, or All Menus at `path` (menu titles
/// and submenu labels from the top; empty: the menu titles).
#[derive(Clone, Debug, PartialEq)]
struct State {
    quick: &'static [(&'static str, &'static str)],
    /// The quick actions need a photo (the editor's).
    needs_photo: bool,
    menus: Option<Vec<String>>,
}

fn state_id() -> egui::Id {
    egui::Id::unique("compact-more-sheet")
}

/// Open the sheet on its first page with `quick` (command id, label) rows.
pub fn open(ctx: &egui::Context, quick: &'static [(&'static str, &'static str)], needs_photo: bool) {
    ctx.data_mut(|d| d.insert_temp(state_id(), State { quick, needs_photo, menus: None }));
}

pub fn is_open(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<State>(state_id()).is_some())
}

fn close(ctx: &egui::Context) {
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
    let mut run: Option<(String, Value)> = None;
    let mut closing = false;
    let modal = egui::Modal::new(egui::Id::unique("compact-more-modal")).area(area).frame(frame).show(ctx, |ui| {
        ui.set_width(width - 24.0);
        register(ui, "panel:moreSheet", ui.max_rect());
        match st.menus.clone() {
            None => first_page(app, ui, &mut st, &mut run, &mut closing),
            Some(path) => menu_page(app, ui, &mut st, &path, &mut run, body_max),
        }
    });
    if modal.should_close() {
        match st.menus.as_mut() {
            // Escape or a tap outside goes back a page, then closes
            Some(path) if !path.is_empty() => {
                path.pop();
            }
            Some(_) => st.menus = None,
            None => closing = true,
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

/// A full-width row: a check mark gutter, the label, and › for a page; true when tapped.
fn row(ui: &mut egui::Ui, id: String, label: &str, enabled: bool, checked: Option<bool>, page: bool) -> bool {
    let t = Tokens::get(ui.ctx());
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(vec2(w, 48.0), if enabled { Sense::click() } else { Sense::hover() });
    register(ui, id, r);
    crate::widgets::announce_selected(&resp, label, checked == Some(true));
    let p = ui.painter();
    if enabled && resp.is_pointer_button_down_on() {
        p.rect_filled(r, 8.0, t.hover);
    }
    let color = if enabled { t.text } else { t.text_disabled };
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
    let (r, _) = ui.allocate_exact_size(vec2(ui.available_width(), 9.0), Sense::hover());
    ui.painter().hline(r.x_range(), r.center().y, Stroke::new(1.0, t.divider));
}

fn first_page(app: &LightcraftApp, ui: &mut egui::Ui, st: &mut State, run: &mut Option<(String, Value)>, closing: &mut bool) {
    if head(ui, false, crate::i18n::tr("More")) {
        *closing = true;
        return;
    }
    let enabled = !st.needs_photo || app.session.active().is_some();
    for (id, label) in st.quick {
        if row(ui, format!("more:{id}"), crate::i18n::tr(label), enabled, None, false) {
            *run = Some(((*id).to_string(), json!({})));
        }
        // the destructive one apart from the rest
        if *id == "dialog.rename" {
            gap(ui);
        }
    }
    // (a host with a native menu bar, the desktop, keeps its own)
    if !app.native_menu {
        gap(ui);
        if row(ui, "more:allMenus".into(), crate::i18n::tr("All Menus"), true, None, true) {
            st.menus = Some(Vec::new());
        }
    }
}

/// All Menus at `path`: the menu titles, or a menu's (or submenu's) items.
fn menu_page(app: &LightcraftApp, ui: &mut egui::Ui, st: &mut State, path: &[String], run: &mut Option<(String, Value)>, body_max: f32) {
    let bar = crate::menubar::menu_bar(app);
    let title = path.last().map_or_else(|| crate::i18n::tr("All Menus").to_string(), |l| crate::i18n::tr(l).to_string());
    if head(ui, true, &title) {
        st.menus = path.split_last().map(|(_, rest)| rest.to_vec()).filter(|_| !path.is_empty());
        return;
    }
    let mut open: Option<String> = None;
    // the page's own height (up to `body_max`), not what the last page left: the sheet's area
    // remembers its size, so a short page would otherwise squeeze the next one into a sliver
    let height = (page_nodes(&bar, path).map_or(bar.len(), <[MenuNode]>::len) as f32 * 48.0).min(body_max);
    // (a scroll state per page)
    egui::ScrollArea::vertical().id_salt(("more-sheet", path)).max_height(body_max).min_scrolled_height(height).auto_shrink([false, true]).show(
        ui,
        |ui| {
            if path.is_empty() {
                for (menu, _) in &bar {
                    if row(ui, format!("menu:{menu}"), crate::i18n::tr(menu), true, None, true) {
                        open = Some(menu.clone());
                    }
                }
                return;
            }
            let Some(nodes) = page_nodes(&bar, path) else { return };
            let mut last_gap = true;
            for n in nodes {
                match n {
                    MenuNode::Separator => {
                        if !last_gap {
                            gap(ui);
                            last_gap = true;
                        }
                    }
                    MenuNode::Submenu { label, children } => {
                        last_gap = false;
                        if row(ui, format!("submenu:{label}"), crate::i18n::tr(label), !children.is_empty(), None, true) {
                            open = Some(label.clone());
                        }
                    }
                    MenuNode::Item { id, params, label, enabled, checked, .. } => {
                        last_gap = false;
                        let shown = crate::menubar::display_item_label(id, params, label);
                        if row(ui, format!("menuitem:{id}"), shown, *enabled, *checked, false) {
                            *run = Some((id.clone(), params.clone()));
                        }
                    }
                }
            }
        },
    );
    if let Some(next) = open {
        let mut p = path.to_vec();
        p.push(next);
        st.menus = Some(p);
    }
}

/// The nodes of the menu at `path` (a menu title, then submenu labels); a submenu that has gone
/// (the menus change with the screen) gives its parent's. None for the titles' page.
fn page_nodes<'a>(bar: &'a [(String, Vec<MenuNode>)], path: &[String]) -> Option<&'a [MenuNode]> {
    let (first, rest) = path.split_first()?;
    let mut nodes = bar.iter().find(|(m, _)| m == first).map(|(_, n)| n.as_slice())?;
    for label in rest {
        match nodes.iter().find_map(|n| match n {
            MenuNode::Submenu { label: l, children } if l == label => Some(children.as_slice()),
            _ => None,
        }) {
            Some(children) => nodes = children,
            None => break,
        }
    }
    Some(nodes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sheet_opens_and_closes() {
        let ctx = egui::Context::default();
        assert!(!is_open(&ctx));
        open(&ctx, &[("app.about", "About LightCraft")], false);
        assert!(is_open(&ctx));
        close(&ctx);
        assert!(!is_open(&ctx));
    }
}
