//! The compact layout: phones, and any window narrower than [`COMPACT_WIDTH`] (iPad Split View, a
//! phone browser) or shorter than [`COMPACT_HEIGHT`] and narrower than [`COMPACT_SHORT_WIDTH`]
//! (a phone held sideways). The desktop
//! layout's side columns and filmstrip leave such a window no room for the photo, so here:
//!
//! - the top bar keeps the sidebar toggle, Back, the menus (one "Menu" button), a search field
//!   that takes the free width, Filter and Export;
//! - the tool strip goes; its tools sit in a row atop a **bottom sheet** under the photo, which
//!   holds the right panel (Edit, Crop, Masking …) or the Presets column at full width; in a
//!   window wider than tall ([`beside`]) the sheet is a column at the right of the photo instead;
//! - My Photos (the left panel) opens as a full-width drawer (a column at the left in a wide
//!   window), closed again by picking a source;
//! - the bottom bar keeps the view modes, Sort and an Edit toggle.
//!
//! The panels themselves are the desktop ones: [`super::resizable_side`] and the Presets column
//! fill the region they're given while [`is_compact`] holds.

use egui::{Rect, Sense, Stroke, StrokeKind, pos2, vec2};
use serde_json::json;

use crate::LightcraftApp;
use crate::icons::{Icon, paint};
use crate::state::{RightPanel, ViewMode};
use crate::theme::Tokens;
use crate::widgets::{icon_button, register};

/// Content width (points, inside the safe area) below which the compact layout is used.
pub const COMPACT_WIDTH: f32 = 600.0;

/// Content height (points, inside the safe area) below which the compact layout is used: a phone
/// in landscape (an iPhone is about 400 pt tall, an Android phone about 410 pt less its status
/// bar). The smallest iPad in landscape is 744 pt tall.
pub const COMPACT_HEIGHT: f32 = 500.0;

/// A short window is compact only when it is also narrower than this: phones in landscape are at
/// most about 960 pt wide, while a short but wide desktop window keeps the desktop layout and its
/// menu bar.
pub const COMPACT_SHORT_WIDTH: f32 = 1000.0;

/// Width of the sheet and the My Photos drawer when they sit beside the photo ([`beside`]).
const SIDE_MIN: f32 = 300.0;
const SIDE_MAX: f32 = 420.0;

/// Side margin of the compact top bar, bottom bar and the sheet's tool row, so their icon
/// columns line up.
const BAR_MARGIN: i8 = 6;

/// Touch-friendly button size in the compact bars.
const BUTTON: egui::Vec2 = egui::vec2(40.0, 40.0);

fn flag_id() -> egui::Id {
    egui::Id::unique("compact-layout")
}

/// Whether this frame uses the compact layout.
pub fn is_compact(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(flag_id())).unwrap_or(false)
}

/// Decide the layout for this frame from the content area (inside the safe area). Entering the
/// compact layout closes My Photos, which would otherwise cover the photo as a full-width
/// drawer. A content area that got shorter (iOS: the soft keyboard came up) starts a few frames
/// of [`keep_focus_visible`].
pub(crate) fn update(app: &mut LightcraftApp, ctx: &egui::Context, content: Rect) {
    let (bottom, frames) = ctx.data(|d| d.get_temp::<(f32, u8)>(shrink_id())).unwrap_or((content.bottom(), 0));
    let frames = if content.bottom() < bottom - 0.5 { 4 } else { frames.saturating_sub(1) };
    ctx.data_mut(|d| d.insert_temp(shrink_id(), (content.bottom(), frames)));
    let compact = content.width() < COMPACT_WIDTH || (content.height() < COMPACT_HEIGHT && content.width() < COMPACT_SHORT_WIDTH);
    if compact && !is_compact(ctx) {
        app.ui.left_panel = false;
    }
    ctx.data_mut(|d| d.insert_temp(flag_id(), compact));
    // a phone: controls a finger can hit
    crate::theme::finger_sizes(ctx, compact && crate::widgets::touch_screen(ctx));
}

/// In the compact layout, whether the window is wider than tall (a phone in landscape): the
/// sheet and the My Photos drawer then sit beside the photo as columns, not over or under it.
pub fn beside(ctx: &egui::Context) -> bool {
    let r = ctx.content_rect();
    is_compact(ctx) && r.width() > r.height()
}

/// The width of a column beside the photo ([`beside`]) in a region `avail` wide.
pub fn side_width(avail: f32) -> f32 {
    (avail * 0.45).clamp(SIDE_MIN, SIDE_MAX).min(avail)
}

/// The bottom sheet is up: Detail view with a right panel or Presets open, or a grid with a
/// panel that isn't an editing tool (Info, Keywords, Versions, History), as the desktop's grid
/// has. (Editing tools and Presets switch to Detail.)
pub fn sheet_open(app: &LightcraftApp) -> bool {
    match app.ui.view {
        ViewMode::Detail => app.ui.right != RightPanel::None || app.ui.presets,
        ViewMode::PhotoGrid | ViewMode::SquareGrid => {
            matches!(app.ui.right, RightPanel::Info | RightPanel::Keywords | RightPanel::Versions | RightPanel::Activity)
        }
        _ => false,
    }
}

/// The window's chrome in the compact layout; the caller fills the central area as usual.
///
/// Laid out like a phone photo editor: the library has a slim top bar (My Photos, Import,
/// search, Filter, Export, and "…" for every menu) over the grid and the bottom bar; the editor
/// (Detail) has Back, Show Original, Undo, Redo, Export and "…" on top, the photo, and a row of
/// labelled tools along the bottom (down the right in landscape). A tool opens its sheet: one
/// Edit section (Light, Color …), the whole Edit panel, Crop, Masking, Presets and the rest.
pub fn show(app: &mut LightcraftApp, ui: &mut egui::Ui) {
    // an Edit section chosen in the tool row applies only while the Edit panel is the one open
    if app.ui.right != RightPanel::Edit {
        set_section(ui.ctx(), None);
    }
    let detail = app.ui.view == ViewMode::Detail;
    // a tap on the photo hid the editor's bars and sheet: the photo takes the screen, but for
    // Crop's, which stays where it is under a cover (`cover_chrome`)
    if !detail || app.session.active().is_none() {
        ui.ctx().data_mut(|d| d.remove_temp::<bool>(chrome_hidden_id()));
    }
    let hidden = detail && chrome_hidden(ui.ctx());
    let cover = hidden && app.ui.right == RightPanel::Crop;
    if hidden && !cover {
        sideways_photo(app, ui.ctx());
        return;
    }
    if detail {
        editor_bar(app, ui);
    } else {
        library_bar(app, ui);
    }
    super::library_problem::banner(app, ui);
    if detail {
        if app.session.active().is_some() {
            tool_dock(app, ui);
        }
    } else {
        bottombar(app, ui);
    }
    if app.ui.left_panel {
        // picking a source (album, folder, date …) closes the drawer onto its photos
        let source = app.session.source;
        // the drawer: the whole region, or a column beside the photos in a wide window
        let avail = ui.available_rect_before_wrap();
        let w = if beside(ui.ctx()) { side_width(avail.width()) } else { avail.width() };
        let r = egui::Panel::left("left_panel").frame(egui::Frame::NONE).resizable(false).exact_size(w).show(ui, |ui| super::left::show(app, ui));
        register(ui, "panel:left_panel", r.response.rect);
        if app.session.source != source {
            app.ui.left_panel = false;
        }
    } else if sheet_open(app) && !(detail && beside(ui.ctx())) {
        // (in landscape the editor's sheet is part of the tool column, drawn by `tool_dock`)
        sheet(app, ui);
    }
    super::share_sheet::show(app, ui.ctx());
    super::filter_sheet::show(app, ui.ctx());
    super::more_sheet::show(app, ui.ctx());
    super::sort_sheet::show(app, ui.ctx());
    super::rate_sheet::show(app, ui.ctx());
    if cover {
        cover_chrome(ui.ctx(), ui.available_rect_before_wrap());
    }
    sideways_photo(app, ui.ctx());
}

/// Whether the phone editor shows its photo alone, turned to fill the screen: the phone is held
/// sideways (its interface stays upright: [`LightcraftApp::device_turn`]), on a photo with no
/// tool's sheet, sheet or dialog open, as phone photo editors do. Turned back, the editor returns.
pub fn sideways(app: &LightcraftApp, ctx: &egui::Context) -> bool {
    app.device_turn != 0
        && app.ui.view == ViewMode::Detail
        && app.session.active().is_some()
        && !sheet_open(app)
        && app.ui.dialog.is_none()
        && !super::share_sheet::is_open(ctx)
        && !super::rate_sheet::is_open(ctx)
        && !super::more_sheet::is_open(ctx)
}

/// The photo alone on black, turned a quarter towards the way the phone is held so it fills the
/// screen ([`sideways`]). The editor goes on underneath (its renders keep coming); this covers it
/// and takes the taps.
fn sideways_photo(app: &LightcraftApp, ctx: &egui::Context) {
    if !sideways(app, ctx) {
        return;
    }
    let Some(id) = app.session.active() else { return };
    let mine = |s: crate::render::Slot| app.renderer.textures.get(&s).filter(|t| t.photo == id);
    let tex = mine(crate::render::Slot::Main).or_else(|| mine(crate::render::Slot::Preview)).or_else(|| app.renderer.thumb(id));
    let screen = ctx.viewport_rect();
    let room = ctx.content_rect();
    egui::Area::new(egui::Id::unique("compact-sideways")).order(egui::Order::Foreground).fixed_pos(screen.min).show(ctx, |ui| {
        let (_, _) = ui.allocate_exact_size(screen.size(), Sense::click_and_drag());
        ui.painter().rect_filled(screen, 0.0, egui::Color32::BLACK);
        let Some(tex) = tex else { return };
        let [w, h] = tex.size.map(|v| v.max(1) as f32);
        // turned a quarter, the photo's width runs down the screen: fit it in the room so
        let scale = (room.height() / w).min(room.width() / h);
        let upright = Rect::from_center_size(room.center(), vec2(w * scale, h * scale));
        // top to the left: the photo's top faces the screen's right (a clockwise quarter)
        let angle = f32::from(app.device_turn.signum()) * std::f32::consts::FRAC_PI_2;
        egui::Image::from_texture(egui::load::SizedTexture::new(tex.tex.id(), upright.size()))
            .rotate(angle, egui::Vec2::splat(0.5))
            .paint_at(ui, upright);
        let shown = Rect::from_center_size(room.center(), vec2(h * scale, w * scale));
        register(ui, "photo:sideways", shown);
    });
}

/// A bar along the top with the safe-area and margins of the compact layout.
fn top_bar(ui: &mut egui::Ui, add: impl FnOnce(&mut egui::Ui)) {
    let t = Tokens::get(ui.ctx());
    egui::Panel::top("top_bar")
        .exact_size(t.top_bar_h + 4.0)
        .frame(egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin { left: BAR_MARGIN, right: BAR_MARGIN, top: 0, bottom: 0 }))
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                add(ui);
            });
        });
}

/// The system has a Back of its own (Android's button or gesture, see [`super::back`]): the bars
/// leave out their ‹.
const SYSTEM_BACK: bool = cfg!(target_os = "android");

/// The right end of a top bar, filled right to left: "…" ([`more_menu`]), then `add`.
fn bar_right(app: &mut LightcraftApp, ui: &mut egui::Ui, editor: bool, add: impl FnOnce(&mut LightcraftApp, &mut egui::Ui)) {
    let rest = ui.available_rect_before_wrap();
    let mut right = ui.new_child(egui::UiBuilder::new().max_rect(rest).layout(egui::Layout::right_to_left(egui::Align::Center)));
    right.spacing_mut().item_spacing.x = 0.0;
    let more = icon_button(&mut right, "more", more_icon(), BUTTON, false, true, "More");
    if app.services.menu_button.is_some() {
        // the host's own menu: its button over this one takes the tap (see `native_menu`)
        crate::native_menu::place(ui.ctx(), more.rect, editor);
    }
    more_menu(app, &more, editor);
    add(app, &mut right);
}

/// "…": the More sheet ([`super::more_sheet`]) with what this screen offers
/// ([`crate::native_menu::more_items`]), as a phone photo editor has it.
fn more_menu(app: &mut LightcraftApp, more: &egui::Response, editor: bool) {
    if !more.clicked() {
        return;
    }
    // (a host with a popup menu of its own opens that: Android's)
    if app.services.open_menu.is_some() {
        let items = crate::native_menu::more_items(app, editor);
        if let Some(open) = app.services.open_menu.as_mut() {
            open(more.rect, &items);
        }
        return;
    }
    super::more_sheet::open(&more.ctx, editor);
}

/// "…" as each system draws it: three dots in a circle on iOS, stacked on Android, in a row
/// elsewhere.
fn more_icon() -> Icon {
    if cfg!(target_os = "ios") {
        Icon::DotsCircle
    } else if cfg!(target_os = "android") {
        Icon::DotsVertical
    } else {
        Icon::Dots
    }
}

/// The About dialog's help on a phone: What's New, Send Feedback, Help and System Info as rows a
/// finger can use (the desktop has them in its Help menu). Nothing in the desktop layout.
pub fn about_help_rows(app: &mut LightcraftApp, ui: &mut egui::Ui) {
    if !is_compact(ui.ctx()) {
        return;
    }
    let mut run = None;
    for (id, label) in
        [("app.whatsNew", "What's New"), ("app.feedback", "Send Feedback…"), ("app.help", "LightCraft Help"), ("app.systemInfo", "System Info…")]
    {
        if super::more_sheet::row(ui, format!("about:{id}"), crate::i18n::tr(label), true, None, false, false) {
            run = Some(id);
        }
    }
    if let Some(id) = run
        && let Err(e) = crate::menubar::run_item(app, id, json!({}))
    {
        app.toast(ui.ctx(), e);
    }
}

fn unsaved_indicator(app: &LightcraftApp, ui: &mut egui::Ui) {
    // a failing save keeps its warning visible (the desktop's cloud icon)
    if let Some((n, e)) = app.session.unsaved() {
        let t = Tokens::get(ui.ctx());
        let (r, resp) = ui.allocate_exact_size(BUTTON, Sense::hover());
        paint(ui.painter(), r.shrink(10.0), Icon::Cloud, t.caution);
        register(ui, "indicator:unsaved", r);
        resp.on_hover_text(format!("{n} unsaved: {e}"));
    }
}

/// The library's bar: My Photos, then (from the right) "…", Share, Filter, Import and the search
/// field in what's left. While the search is being typed in, the field takes the whole bar with
/// Cancel after it, as a phone's search does.
fn library_bar(app: &mut LightcraftApp, ui: &mut egui::Ui) {
    if search_open(ui.ctx()) {
        top_bar(ui, |ui| {
            let rest = ui.available_rect_before_wrap();
            let mut right = ui.new_child(egui::UiBuilder::new().max_rect(rest).layout(egui::Layout::right_to_left(egui::Align::Center)));
            let t = Tokens::get(ui.ctx());
            let cancel = right.add(
                egui::Button::new(egui::RichText::new(crate::i18n::tr("Cancel")).font(t.font(15.0)).color(t.accent))
                    .frame(false)
                    .min_size(vec2(64.0, 40.0)),
            );
            register(&right, "button:searchCancel", cancel.rect);
            if cancel.clicked() {
                // as on a phone: Cancel drops the search and puts the keyboard away
                set_search_open(ui.ctx(), false);
                right.memory_mut(|m| m.surrender_focus(crate::text_field::id("field:search")));
                if !app.ui.search.is_empty() {
                    app.ui.search.clear();
                    let _ = app.run("library.filter", json!({"text": ""}));
                }
                return;
            }
            right.add_space(4.0);
            let field = right.available_rect_before_wrap();
            search(app, &mut right, Rect::from_center_size(field.center(), vec2(field.width(), 34.0)));
        });
        return;
    }
    top_bar(ui, |ui| {
        if icon_button(ui, "sidebar", Icon::Sidebar, BUTTON, app.ui.left_panel, true, "Show/hide My Photos panel").clicked() {
            let _ = app.run("view.leftPanel", json!({}));
        }
        if !SYSTEM_BACK
            && app.ui.view != ViewMode::PhotoGrid
            && app.ui.view != ViewMode::SquareGrid
            && icon_button(ui, "back", Icon::Back, BUTTON, false, true, "Back").clicked()
        {
            let _ = app.run("view.back", json!({}));
        }
        bar_right(app, ui, false, |app, right| {
            let any = app.session.active().is_some() || !app.session.selection.ids.is_empty();
            if icon_button(right, "share", Icon::Share, BUTTON, false, any, "Share").clicked() {
                super::share_sheet::open(right.ctx(), super::share_sheet::photos_to_share(app, false));
            }
            // Filter sits on the album row; a folder's own header (Local) has no room for it
            if !album_row(app) {
                filter_button(app, right);
            }
            // the system photo picker (or the Files app's) on a phone; the file dialog elsewhere
            if icon_button(right, "import", Icon::Plus, BUTTON, false, true, "Import Photos…").clicked()
                && let Err(e) = crate::menubar::run_item(app, "file.addPhotos", json!({}))
            {
                app.toast(right.ctx(), e);
            }
            unsaved_indicator(app, right);
            right.add_space(4.0);
            let field = right.available_rect_before_wrap();
            if field.width() >= 80.0 {
                search(app, right, Rect::from_center_size(field.center(), vec2(field.width(), 34.0)));
            }
        });
    });
}

/// Width of Filter and Sort at the right of the album row ([`album_tools`]).
pub const ALBUM_TOOLS_W: f32 = 2.0 * BUTTON.x + 4.0;

/// Whether the grid's header is the album row (title and count), which carries Filter and Sort,
/// rather than a Local folder's path bar.
fn album_row(app: &LightcraftApp) -> bool {
    app.session.browse.is_none() || app.session.source != lightcraft_engine::LibrarySource::Folder
}

/// Filter, with the number of filters on as a badge; it opens the Filter sheet.
fn filter_button(app: &LightcraftApp, ui: &mut egui::Ui) {
    let on = super::filter_sheet::active_count(app);
    let filter = icon_button(ui, "filter", Icon::Filter, BUTTON, on > 0, true, "Filter");
    if on > 0 {
        badge(ui, filter.rect, on);
    }
    if filter.clicked() {
        super::filter_sheet::open(ui.ctx());
    }
}

/// Filter and Sort at the right end of the grid's album row, `r` ([`ALBUM_TOOLS_W`] wide), next to
/// what they act on.
pub fn album_tools(app: &mut LightcraftApp, ui: &mut egui::Ui, r: Rect) {
    let mut row = ui.new_child(egui::UiBuilder::new().max_rect(r).layout(egui::Layout::right_to_left(egui::Align::Center)));
    row.spacing_mut().item_spacing.x = 4.0;
    if icon_button(&mut row, "sort", Icon::Sort, BUTTON, false, true, "Sort").clicked() {
        super::sort_sheet::open(row.ctx());
    }
    filter_button(app, &mut row);
}

/// How many filters are on, as a small dot with the number at the top right of `r`.
fn badge(ui: &egui::Ui, r: Rect, n: usize) {
    let t = Tokens::get(ui.ctx());
    let c = pos2(r.center().x + 11.0, r.center().y - 11.0);
    ui.painter().circle(c, 8.0, t.accent, Stroke::new(1.5, t.chrome));
    let text = if n > 9 { "9+".to_string() } else { n.to_string() };
    ui.painter().text(c, egui::Align2::CENTER_CENTER, text, t.semibold(10.0), egui::Color32::WHITE);
    register(ui, "indicator:filterCount", Rect::from_center_size(c, vec2(16.0, 16.0)));
}

fn search_open_id() -> egui::Id {
    egui::Id::unique("compact-search-open")
}

/// Whether the search takes the whole bar: from when its field gets the keyboard until it gives it
/// up (Return, a tap elsewhere) or Cancel. The flag outlives the frame the field loses focus on,
/// so a tap on Cancel (which takes the focus as the finger goes down) still lands on Cancel.
fn search_open(ctx: &egui::Context) -> bool {
    let focused = ctx.memory(|m| m.has_focus(crate::text_field::id("field:search")));
    let was = ctx.data(|d| d.get_temp::<bool>(search_open_id())).unwrap_or(false);
    let open = focused || (was && ctx.input(|i| i.pointer.any_down() || i.pointer.any_released()));
    if open != was {
        set_search_open(ctx, open);
    }
    open
}

fn set_search_open(ctx: &egui::Context, open: bool) {
    ctx.data_mut(|d| d.insert_temp(search_open_id(), open));
}

/// The system's Back while the search takes the bar: as Cancel, the keyboard goes away (the text
/// stays). False: it wasn't open.
pub(crate) fn close_search(ctx: &egui::Context) -> bool {
    if !search_open(ctx) {
        return false;
    }
    set_search_open(ctx, false);
    ctx.memory_mut(|m| m.surrender_focus(crate::text_field::id("field:search")));
    true
}

/// The search field in `r` (same id and command as the desktop top bar's): the magnifier, the text
/// and, once there is some, ✕ to clear it.
fn search(app: &mut LightcraftApp, ui: &mut egui::Ui, r: Rect) {
    let t = Tokens::get(ui.ctx());
    let id = crate::text_field::id("field:search");
    if std::mem::take(&mut app.ui.focus_search) {
        ui.memory_mut(|m| m.request_focus(id));
    }
    let focused = ui.memory(|m| m.has_focus(id));
    ui.painter().rect(
        r,
        17.0,
        if focused { t.canvas } else { t.field },
        Stroke::new(1.0, if focused { t.accent } else { t.field_border }),
        StrokeKind::Inside,
    );
    let inner = r.shrink2(vec2(12.0, 4.0));
    paint(ui.painter(), Rect::from_min_size(pos2(inner.left(), r.center().y - 8.0), vec2(16.0, 16.0)), Icon::Search, t.text_dim);
    let has_text = !app.ui.search.is_empty();
    let clear_r = Rect::from_center_size(pos2(r.right() - 17.0, r.center().y), vec2(34.0, 34.0));
    let right = if has_text { clear_r.left() } else { inner.right() };
    // one line tall and centred in the field: a touch screen's finger-sized controls would make the
    // text box taller than the field and set its text (and the hint) above the magnifier
    let font = t.font(15.0);
    let line = ui.painter().layout_no_wrap("Ag".to_string(), font.clone(), t.text).size().y;
    let text = Rect::from_min_max(pos2(inner.left() + 24.0, r.center().y - line / 2.0), pos2(right, r.center().y + line / 2.0));
    let mut cleared = false;
    if has_text {
        let resp = ui.interact(clear_r, ui.scope_id().with("search-clear"), Sense::click());
        register(ui, "button:searchClear", clear_r);
        let p = ui.painter();
        p.circle_filled(clear_r.center(), 8.0, t.text_dim);
        paint(p, Rect::from_center_size(clear_r.center(), vec2(10.0, 10.0)), Icon::Close, t.field);
        if resp.clicked() {
            app.ui.search.clear();
            cleared = true;
        }
    }
    let mut child = ui.new_child(egui::UiBuilder::new().max_rect(text).layout(egui::Layout::left_to_right(egui::Align::Center)));
    child.spacing_mut().interact_size.y = line;
    // (registered as `field:search` by the field itself, like the desktop's)
    let resp = crate::text_field::TextField::singleline("field:search", &mut app.ui.search)
        .frame(false)
        .margin(egui::Margin::ZERO)
        .hint(crate::i18n::tr("Search photos"))
        .width(text.width())
        .font(font)
        .text_color(t.text)
        .show(&mut child)
        .response;
    if cleared {
        // after the field, which gives the keyboard up on a tap outside it (the ✕ is outside)
        ui.memory_mut(|m| m.request_focus(id));
    }
    if resp.changed() || cleared {
        let q = app.ui.search.clone();
        let _ = app.run("library.filter", json!({"text": q}));
    }
}

/// The editor's bar: Back to the photos, then (from the right) "…", Share, Rate, Redo, Undo and
/// Show Original (held down: the photo as shot, let go: the edit).
fn editor_bar(app: &mut LightcraftApp, ui: &mut egui::Ui) {
    top_bar(ui, |ui| {
        if !SYSTEM_BACK && icon_button(ui, "back", Icon::ChevronLeft, BUTTON, false, true, "Back").clicked() {
            let _ = app.run("view.back", json!({}));
        }
        bar_right(app, ui, true, |app, right| {
            if icon_button(right, "share", Icon::Share, BUTTON, false, app.session.active().is_some(), "Share").clicked() {
                super::share_sheet::open(right.ctx(), super::share_sheet::photos_to_share(app, true));
            }
            let active = app.session.active();
            let rated = active.and_then(|id| app.session.catalog.photo(id)).is_some_and(|p| p.rating > 0);
            if icon_button(right, "rate", if rated { Icon::StarFilled } else { Icon::Star }, BUTTON, false, active.is_some(), "Rate").clicked() {
                super::rate_sheet::open(right.ctx(), active.into_iter().collect());
            }
            if undo_button(right, "redo", "Redo", true).clicked() {
                let _ = app.run("edit.redo", json!({}));
            }
            if undo_button(right, "undo", "Undo", false).clicked() {
                let _ = app.run("edit.undo", json!({}));
            }
            let r = icon_button(right, "showOriginal", Icon::BeforeAfter, BUTTON, false, true, "Show Original (hold)");
            hold_original(app, &r);
            unsaved_indicator(app, right);
        });
    });
}

/// Show Original held down: the press shows the photo as shot, and the release brings back the
/// view it replaced. Only the press and release change the view, so View > Show Original and the
/// Before / After layouts work as usual otherwise. The hold is followed until the pointer lifts,
/// not by the button's response: egui turns a finger held past `max_click_duration` into a long
/// press and the button then no longer reports the pointer as down on it.
fn hold_original(app: &mut LightcraftApp, r: &egui::Response) {
    let key = egui::Id::unique("compact-hold-original");
    let was: Option<crate::state::BeforeAfter> = r.ctx.data(|d| d.get_temp(key));
    match was {
        None if r.is_pointer_button_down_on() => {
            r.ctx.data_mut(|d| d.insert_temp(key, app.ui.before_after));
            app.ui.before_after = crate::state::BeforeAfter::Original;
        }
        Some(before) if !r.ctx.input(|i| i.pointer.any_down() || i.any_touches()) => {
            r.ctx.data_mut(|d| d.remove::<crate::state::BeforeAfter>(key));
            app.ui.before_after = before;
        }
        _ => {}
    }
}

/// Undo (a hooked arrow turning back to the left) or Redo (the same, mirrored).
fn undo_button(ui: &mut egui::Ui, id: &str, tip: &str, redo: bool) -> egui::Response {
    let r = icon_button(ui, id, Icon::Circle, BUTTON, false, true, tip);
    // paint over the placeholder glyph: there is no undo icon in the set
    let t = Tokens::get(ui.ctx());
    let c = r.rect.center();
    ui.painter().rect_filled(Rect::from_center_size(c, vec2(26.0, 26.0)), 4.0, if r.hovered() { t.hover } else { t.chrome });
    let sx = if redo { -1.0 } else { 1.0 };
    let p = |x: f32, y: f32| pos2(c.x + sx * x, c.y + y);
    let stroke = Stroke::new(1.6, t.icon);
    // the hook: from the arrow head, across, round and back down
    let mut pts = vec![p(-7.0, -3.0)];
    for i in 0..=8 {
        let a = std::f32::consts::FRAC_PI_2 * (f32::from(i as u8) / 8.0) * 2.0 - std::f32::consts::FRAC_PI_2;
        pts.push(p(1.5 + 5.0 * a.cos(), 2.0 + 5.0 * a.sin()));
    }
    pts.push(p(-1.0, 7.0));
    ui.painter().add(egui::Shape::line(pts, stroke));
    ui.painter().line_segment([p(-7.0, -3.0), p(-3.5, -6.5)], stroke);
    ui.painter().line_segment([p(-7.0, -3.0), p(-3.5, 0.5)], stroke);
    r
}

/// The library's bottom bar on a phone: only Select mode's ([`select_bar`]). Otherwise there is
/// none: a pinch sets the grid's size, a tap on a photo opens the editor, a long press (or "…" →
/// Select Photos) starts Select mode, and Sort sits on the album row.
fn bottombar(app: &mut LightcraftApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    if app.ui.select_mode {
        egui::Panel::bottom("bottom_bar")
            .exact_size(t.bottom_bar_h + 4.0)
            .frame(egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin { left: BAR_MARGIN + 6, right: BAR_MARGIN, top: 0, bottom: 0 }))
            .show(ui, |ui| select_bar(app, ui));
    }
}

/// A frameless text button of the select bar, as tall as a finger; true when tapped.
fn bar_text_button(ui: &mut egui::Ui, id: &str, label: &str, color: egui::Color32, enabled: bool) -> bool {
    let t = Tokens::get(ui.ctx());
    let color = if enabled { color } else { t.text_disabled };
    let b = ui.add_enabled(
        enabled,
        egui::Button::new(egui::RichText::new(crate::i18n::tr(label)).font(t.semibold(14.0)).color(color)).frame(false).min_size(vec2(0.0, 44.0)),
    );
    register(ui, format!("button:{id}"), b.rect);
    b.clicked()
}

/// Select mode's bar: how many photos are selected and Deselect All, then Rate, Album, Move to
/// Folder, Delete, and Done (leaves Select mode; not on Android, whose Back does). In Recently Deleted, Restore and Delete
/// Permanently take the place of the others. Share is the top bar's, which shares the selection.
fn select_bar(app: &mut LightcraftApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let n = app.session.selection.ids.len();
    let any = n > 0;
    let trash = app.session.source == lightcraft_engine::LibrarySource::RecentlyDeleted || crate::menubar::selection_deleted(app);
    let full = ui.available_rect_before_wrap();
    // the buttons first, from the right edge: what they leave is the count's and Deselect All's
    let mut right = ui.new_child(egui::UiBuilder::new().max_rect(full).layout(egui::Layout::right_to_left(egui::Align::Center)));
    right.spacing_mut().item_spacing.x = 2.0;
    select_actions(app, &mut right, any, trash);
    let room = Rect::from_min_max(full.min, pos2((right.min_rect().left() - 6.0).max(full.left()), full.bottom()));
    let mut left = ui.new_child(egui::UiBuilder::new().max_rect(room).layout(egui::Layout::left_to_right(egui::Align::Center)));
    left.spacing_mut().item_spacing.x = 2.0;
    let count = if any { crate::i18n::tr_format!("{n} selected", n = n) } else { crate::i18n::tr("Select Photos").to_string() };
    let font = t.semibold(14.0);
    let width = |s: &str| left.painter().layout_no_wrap(s.to_string(), font.clone(), t.text).size().x;
    // (with less room, Deselect All is a ✕ after the count; with less still, the count is cut)
    let deselect = crate::i18n::tr("Deselect All");
    let words = any && width(&count) + 6.0 + width(deselect) + 16.0 <= room.width();
    let mark = if any && !words { BUTTON.x } else { 0.0 };
    let shown = crate::widgets::elide_head(&count, (room.width() - mark - 4.0).max(0.0), width);
    left.label(egui::RichText::new(shown).font(font.clone()).color(t.text));
    let none = if words {
        left.add_space(4.0);
        bar_text_button(&mut left, "selectNone", "Deselect All", t.accent, true)
    } else {
        any && icon_button(&mut left, "selectNone", Icon::Close, BUTTON, false, true, "Deselect All").clicked()
    };
    if none {
        let _ = crate::menubar::run_item(app, "library.selectNone", json!({}));
    }
    ui.allocate_rect(full, Sense::hover());
}

/// Select mode's buttons, right to left: Done (not on Android, whose Back leaves Select mode),
/// then Delete, Move to Folder, Rate and Add to Album, or in Recently Deleted Restore and Delete
/// Permanently.
fn select_actions(app: &mut LightcraftApp, right: &mut egui::Ui, any: bool, trash: bool) {
    let t = Tokens::get(right.ctx());
    if !SYSTEM_BACK {
        let done = right.add(egui::Button::new(egui::RichText::new(crate::i18n::tr("Done")).font(t.semibold(14.0)).color(t.accent)).frame(false));
        register(right, "button:selectDone", done.rect);
        if done.clicked() {
            let _ = app.run("view.selectMode", json!({"on": false}));
        }
    }
    if trash {
        // the photos are deleted already: put them back, or remove them for good
        if trash_button(right, any) {
            let _ = crate::menubar::run_item(app, "photo.deletePermanently", json!({}));
        }
        if bar_text_button(right, "selectRestore", "Restore", t.accent, any) {
            let _ = crate::menubar::run_item(app, "photo.restore", json!({}));
        }
        return;
    }
    if icon_button(right, "selectDelete", Icon::Trash, BUTTON, false, any, "Delete").clicked() {
        let _ = crate::menubar::run_item(app, "photo.delete", json!({}));
    }
    if crate::panels::grid::can_move_to_folder(app) && icon_button(right, "selectMove", Icon::Folder, BUTTON, false, any, "Move to Folder…").clicked()
    {
        crate::panels::grid::move_to_folder(app, right.ctx());
    }
    if icon_button(right, "selectRate", Icon::Star, BUTTON, false, any, "Rate").clicked() {
        super::rate_sheet::open(right.ctx(), super::share_sheet::photos_to_share(app, false));
    }
    let album = icon_button(right, "selectAlbum", Icon::Album, BUTTON, false, any, "Add to Album");
    if let Some(id) = app.session.selection.active.or_else(|| app.session.selection.ids.first().copied()) {
        egui::Popup::menu(&album).show(|ui| crate::panels::grid::context_menu_albums(app, ui, id));
    }
}

/// Delete Permanently: the trash in red (it can be undone, but not from Recently Deleted).
fn trash_button(ui: &mut egui::Ui, enabled: bool) -> bool {
    let t = Tokens::get(ui.ctx());
    let (r, resp) = ui.allocate_exact_size(vec2(BUTTON.x, 44.0), if enabled { Sense::click() } else { Sense::hover() });
    register(ui, "icon:selectDeletePermanently", r);
    let label = crate::i18n::tr("Delete Permanently");
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, label));
    if enabled && resp.is_pointer_button_down_on() {
        ui.painter().rect_filled(r.shrink(2.0), 8.0, t.hover);
    }
    paint(ui.painter(), Rect::from_center_size(r.center(), vec2(20.0, 20.0)), Icon::Trash, if enabled { t.reject } else { t.text_disabled });
    let clicked = enabled && resp.clicked();
    resp.on_hover_text(label);
    clicked
}

fn close_sheet(app: &mut LightcraftApp) {
    app.ui.right = RightPanel::None;
    app.ui.presets = false;
}

fn chrome_hidden_id() -> egui::Id {
    egui::Id::unique("compact-editor-chrome-hidden")
}

/// Whether a tap on the photo hid the editor's bars and sheet (as a phone photo editor does).
pub fn chrome_hidden(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<bool>(chrome_hidden_id())).unwrap_or(false)
}

/// A tap on the photo in the editor: its bars and sheet go, or come back.
pub(crate) fn toggle_chrome(ctx: &egui::Context) {
    let hidden = !chrome_hidden(ctx);
    ctx.data_mut(|d| d.insert_temp(chrome_hidden_id(), hidden));
}

/// Crop with the editor's chrome hidden: the bars and sheet keep their room, so the photo stays
/// where it is, and are covered with the canvas's colour; a tap on them brings them back.
/// `photo`: the room left for the photo.
fn cover_chrome(ctx: &egui::Context, photo: Rect) {
    let t = Tokens::get(ctx);
    let screen = ctx.viewport_rect();
    let parts = [
        Rect::from_min_max(screen.min, egui::pos2(screen.right(), photo.top())),
        Rect::from_min_max(egui::pos2(screen.left(), photo.bottom()), screen.max),
        Rect::from_min_max(egui::pos2(screen.left(), photo.top()), egui::pos2(photo.left(), photo.bottom())),
        Rect::from_min_max(egui::pos2(photo.right(), photo.top()), egui::pos2(screen.right(), photo.bottom())),
    ];
    for (i, r) in parts.into_iter().enumerate() {
        if r.width() < 0.5 || r.height() < 0.5 {
            continue;
        }
        egui::Area::new(egui::Id::unique("compact-chrome-cover").with(i)).order(egui::Order::Foreground).fixed_pos(r.min).show(ctx, |ui| {
            let (rect, resp) = ui.allocate_exact_size(r.size(), Sense::click());
            ui.painter().rect_filled(rect, 0.0, t.canvas);
            if resp.clicked() {
                toggle_chrome(ctx);
            }
        });
    }
}

/// What a tab of the editor's tool row opens.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    Presets,
    /// An action, not a sheet: Auto settings.
    Auto,
    /// A right panel (Edit shows all of its sections).
    Panel(RightPanel),
    /// One section of the Edit panel on its own.
    Section(&'static str),
}

/// The editor's tool row, in a phone photo editor's order: (widget id, label, icon, tab).
const TABS: &[(&str, &str, Icon, Tab)] = &[
    ("presets", "Presets", Icon::Presets, Tab::Presets),
    ("auto", "Auto", Icon::Target, Tab::Auto),
    ("crop", "Crop", Icon::Crop, Tab::Panel(RightPanel::Crop)),
    ("light", "Light", Icon::Sliders, Tab::Section("light")),
    ("color", "Color", Icon::Radial, Tab::Section("color")),
    ("effects", "Effects", Icon::Curve, Tab::Section("effects")),
    ("detailSection", "Detail", Icon::Picker, Tab::Section("detail")),
    ("optics", "Optics", Icon::Circle, Tab::Section("optics")),
    ("masking", "Masking", Icon::Mask, Tab::Panel(RightPanel::Masking)),
    ("remove", "Remove", Icon::Eraser, Tab::Panel(RightPanel::Remove)),
    ("edit", "All", Icon::Pencil, Tab::Panel(RightPanel::Edit)),
    ("calibration", "Calibration", Icon::ProfileGrid, Tab::Section("calibration")),
    ("redeye", "Red Eye", Icon::Eye, Tab::Panel(RightPanel::RedEye)),
    ("info", "Info", Icon::Info, Tab::Panel(RightPanel::Info)),
    ("keywords", "Keywords", Icon::Tag, Tab::Panel(RightPanel::Keywords)),
    ("versions", "Versions", Icon::Versions, Tab::Panel(RightPanel::Versions)),
    ("activity", "History", Icon::Activity, Tab::Panel(RightPanel::Activity)),
];

/// A tab's touch target.
const TAB: egui::Vec2 = egui::vec2(62.0, 54.0);

fn section_id() -> egui::Id {
    egui::Id::unique("compact-edit-section")
}

/// The Edit section the sheet shows on its own (`None`: the whole Edit panel).
fn section(ctx: &egui::Context) -> Option<&'static str> {
    ctx.data(|d| d.get_temp::<Option<&'static str>>(section_id())).flatten()
}

fn set_section(ctx: &egui::Context, s: Option<&'static str>) {
    ctx.data_mut(|d| d.insert_temp(section_id(), s));
}

fn tab_on(app: &LightcraftApp, ctx: &egui::Context, tab: Tab) -> bool {
    match tab {
        Tab::Presets => app.ui.presets,
        Tab::Auto => false,
        Tab::Panel(RightPanel::Edit) => !app.ui.presets && matches!(app.ui.right, RightPanel::Edit | RightPanel::Profiles) && section(ctx).is_none(),
        Tab::Panel(p) => !app.ui.presets && app.ui.right == p,
        Tab::Section(k) => !app.ui.presets && app.ui.right == RightPanel::Edit && section(ctx) == Some(k),
    }
}

/// A tab pressed: its sheet opens (one thing at a time), or, pressed again, the sheet goes away.
fn press(app: &mut LightcraftApp, ctx: &egui::Context, tab: Tab) {
    if tab == Tab::Auto {
        let _ = app.run("develop.auto", json!({}));
        app.toast(ctx, crate::i18n::tr("Auto settings applied"));
        return;
    }
    if tab_on(app, ctx, tab) {
        close_sheet(app);
        return;
    }
    match tab {
        Tab::Presets => {
            if !app.ui.presets {
                let _ = app.run("panel.presets", json!({}));
            }
        }
        Tab::Panel(p) => {
            app.ui.presets = false;
            set_section(ctx, None);
            if app.ui.right != p {
                let id = TABS.iter().find(|(_, _, _, t)| *t == tab).map_or("edit", |(id, ..)| *id);
                let _ = app.run(&format!("panel.{id}"), json!({}));
            }
        }
        Tab::Section(k) => {
            app.ui.presets = false;
            if app.ui.right != RightPanel::Edit {
                let _ = app.run("panel.edit", json!({}));
            }
            set_section(ctx, Some(k));
        }
        Tab::Auto => {}
    }
}

/// Press the tool row's tab `id` (`icon:{id}`), wherever the row is scrolled (tests).
#[cfg(test)]
pub(crate) fn press_tab(app: &mut LightcraftApp, ctx: &egui::Context, id: &str) {
    if let Some((.., tab)) = TABS.iter().find(|(i, ..)| *i == id) {
        press(app, ctx, *tab);
    }
}

/// The tool row (or, `vertical`, column): each tab an icon over its name, the open one lit.
fn tool_tabs(app: &mut LightcraftApp, ui: &mut egui::Ui, vertical: bool) {
    let t = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    let mut row = |ui: &mut egui::Ui| {
        for (id, label, icon, tab) in TABS {
            let on = tab_on(app, &ctx, *tab);
            let (r, resp) = ui.allocate_exact_size(TAB, Sense::click());
            register(ui, format!("icon:{id}"), r);
            resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::Button, true, on, crate::i18n::tr(label)));
            let color = if on { t.accent } else { t.icon };
            if on {
                ui.painter().rect_filled(r.shrink(2.0), 8.0, t.tool_active);
            }
            paint(ui.painter(), Rect::from_center_size(pos2(r.center().x, r.top() + 19.0), vec2(20.0, 20.0)), *icon, color);
            ui.painter().text(pos2(r.center().x, r.bottom() - 11.0), egui::Align2::CENTER_CENTER, crate::i18n::tr(label), t.font(10.5), color);
            if resp.clicked() {
                press(app, &ctx, *tab);
            }
        }
    };
    // dragged by the finger anywhere on the row: no scroll bar to grab as well (a phone's tool
    // rows have none)
    let drag = egui::scroll_area::ScrollSource { drag: egui::scroll_area::DragScroll::Always, scroll_bar: false, ..Default::default() };
    let hidden = egui::scroll_area::ScrollBarVisibility::AlwaysHidden;
    if vertical {
        egui::ScrollArea::vertical().id_salt("compact-tools").scroll_source(drag).scroll_bar_visibility(hidden).auto_shrink([false, false]).show(
            ui,
            |ui| {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 2.0;
                    row(ui);
                });
            },
        );
    } else {
        egui::ScrollArea::horizontal().id_salt("compact-tools").scroll_source(drag).scroll_bar_visibility(hidden).auto_shrink([false, false]).show(
            ui,
            |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = 2.0;
                    row(ui);
                });
            },
        );
    }
}

/// The editor's tools: a row along the bottom, or in landscape a column at the right edge (with
/// the open sheet beside it, as one column).
fn tool_dock(app: &mut LightcraftApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    if beside(ui.ctx()) {
        let avail = ui.available_rect_before_wrap();
        if sheet_open(app) && !app.ui.left_panel {
            let r = egui::Panel::right("compact_sheet_side")
                .exact_size(side_width(avail.width()))
                .resizable(false)
                .frame(egui::Frame::NONE.fill(t.chrome).stroke(Stroke::new(1.0, t.divider)))
                .show(ui, |ui| {
                    register(ui, "panel:sheet", ui.max_rect());
                    egui::Panel::bottom("compact_tools")
                        .exact_size(TAB.y + 6.0)
                        .resizable(false)
                        .frame(egui::Frame::NONE.inner_margin(egui::Margin { left: BAR_MARGIN, right: BAR_MARGIN, top: 2, bottom: 2 }))
                        .show(ui, |ui| tool_tabs(app, ui, false));
                    sheet_body(app, ui);
                });
            register(ui, "panel:tools", r.response.rect);
        } else {
            let r = egui::Panel::right("compact_tools_side")
                .exact_size(TAB.x + 8.0)
                .resizable(false)
                .frame(egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin { left: 4, right: 4, top: 0, bottom: 0 }))
                .show(ui, |ui| tool_tabs(app, ui, true));
            register(ui, "panel:tools", r.response.rect);
        }
    } else {
        let r = egui::Panel::bottom("compact_tools")
            .exact_size(TAB.y + 8.0)
            .resizable(false)
            .frame(egui::Frame::NONE.fill(t.chrome).inner_margin(egui::Margin { left: BAR_MARGIN, right: BAR_MARGIN, top: 3, bottom: 3 }))
            .show(ui, |ui| tool_tabs(app, ui, false));
        register(ui, "panel:tools", r.response.rect);
    }
}

/// The bottom sheet above the tool row (in landscape, the editor's sheet is part of the tool
/// column instead; the grid's Info, Keywords … sheet is a column at the right there).
fn sheet(app: &mut LightcraftApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let avail = ui.available_rect_before_wrap();
    let panel = if beside(ui.ctx()) {
        egui::Panel::right("compact_sheet_side").exact_size(side_width(avail.width()))
    } else {
        // the photo keeps the upper part; the sheet gets the rest, within reason
        let height = (avail.height() * 0.5).clamp(200.0, (avail.height() - 160.0).max(200.0));
        egui::Panel::bottom("compact_sheet").exact_size(height)
    };
    panel.resizable(false).frame(egui::Frame::NONE.fill(t.chrome).stroke(Stroke::new(1.0, t.divider))).show(ui, |ui| {
        register(ui, "panel:sheet", ui.max_rect());
        sheet_body(app, ui);
    });
}

/// The sheet's title and Hide, then what the open tool shows.
fn sheet_body(app: &mut LightcraftApp, ui: &mut egui::Ui) {
    let t = Tokens::get(ui.ctx());
    let ctx = ui.ctx().clone();
    let title = TABS.iter().find(|(.., tab)| tab_on(app, &ctx, *tab)).map_or("Edit", |(_, label, ..)| *label);
    // one Edit section: its eye (on / off), as on the section's header in the full panel
    let eye = section(&ctx)
        .filter(|_| app.ui.right == RightPanel::Edit && !app.ui.presets)
        .zip(app.session.active())
        .map(|(key, id)| (key, super::edit::shown_settings(app, id).0.section_enabled(key)));
    egui::Panel::top("compact_sheet_head")
        .exact_size(36.0)
        .resizable(false)
        .frame(egui::Frame::NONE.inner_margin(egui::Margin { left: 16, right: BAR_MARGIN - 1, top: 0, bottom: 0 }))
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                ui.label(egui::RichText::new(crate::i18n::tr(title)).font(t.semibold(14.0)).color(t.text));
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if icon_button(ui, "sheetClose", Icon::ChevronDown, vec2(36.0, 36.0), false, true, "Hide").clicked() {
                        close_sheet(app);
                    }
                    if let Some((key, on)) = eye {
                        section_eye(app, ui, key, on);
                    }
                    // Presets: + and "…" here, beside the title, not in a strip of their own
                    if app.ui.presets {
                        super::presets::header_buttons(app, ui, 36.0);
                    }
                });
            });
        });
    if app.ui.presets {
        super::presets::show(app, ui);
        return;
    }
    let one = section(&ctx).filter(|_| app.ui.right == RightPanel::Edit);
    match (one, app.session.active()) {
        (Some(key), Some(id)) => {
            register(ui, "panel:right_panel", ui.available_rect_before_wrap());
            let drag = egui::scroll_area::ScrollSource { drag: egui::scroll_area::DragScroll::Always, ..Default::default() };
            egui::ScrollArea::vertical().id_salt("compact-section").scroll_source(drag).auto_shrink([false, false]).show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.add_space(6.0);
                super::edit::notices(app, ui, id);
                let (d, raw) = super::edit::shown_settings(app, id);
                super::edit::section_body(app, ui, &d, id, raw, key);
                ui.add_space(24.0);
                keep_focus_visible(ui);
            });
        }
        _ => {
            register(ui, "panel:right_panel", ui.available_rect_before_wrap());
            // a tool's panel (Crop, Masking, Remove, Info …): the desktop's, with a finger's
            // controls, dropdown lists included
            finger_style(ui);
            let before = crate::widgets::set_finger_scope(true);
            super::right::show(app, ui);
            crate::widgets::set_finger_scope(before);
        }
    }
}

/// The sheet's eye for its Edit section: off, the section contributes nothing to the photo.
fn section_eye(app: &mut LightcraftApp, ui: &mut egui::Ui, key: &str, on: bool) {
    let t = Tokens::get(ui.ctx());
    let tip = if on { "Hide this section's edits" } else { "Show this section's edits" };
    let r = icon_button(ui, "sheetEye", Icon::Eye, vec2(36.0, 36.0), false, true, tip);
    crate::widgets::announce_selected(&r, crate::i18n::tr("Show / hide"), on);
    if !on {
        let e = Rect::from_center_size(r.rect.center(), vec2(18.0, 18.0));
        ui.painter().line_segment([e.left_bottom(), e.right_top()], Stroke::new(1.5, t.text_disabled));
    }
    if r.clicked() {
        let _ = app.run("develop.sectionEnabled", json!({"section": key, "enabled": !on}));
    }
}

fn shrink_id() -> egui::Id {
    egui::Id::unique("content-shrink")
}

/// Call at the end of a scroll area's contents: for a few frames after the content area got
/// shorter, the focused widget (a text field, typically, whose keyboard just came up) is scrolled
/// back into view if it's in this scroll area and now hidden below the content area.
pub fn keep_focus_visible(ui: &egui::Ui) {
    let ctx = ui.ctx();
    // not on the frame the area shrank: the focused widget's rectangle (from the previous frame)
    // is still where it was laid out before
    if ctx.data(|d| d.get_temp::<(f32, u8)>(shrink_id())).is_none_or(|(_, frames)| frames == 0 || frames == 4) {
        return;
    }
    let Some(focused) = ctx.memory(|m| m.focused()).and_then(|id| ctx.read_response(id)) else { return };
    let visible = ui.clip_rect().intersect(ctx.content_rect());
    if ui.min_rect().contains_rect(focused.rect) && !visible.contains_rect(focused.rect) {
        // at once: the layout is still settling, and an animation would restart every frame
        ui.scroll_to_rect_animation(focused.rect, Some(egui::Align::Center), egui::style::ScrollAnimation::none());
    }
}

/// Space kept between a compact-layout dialog and the safe area's edges.
const DIALOG_INSET: f32 = 8.0;

/// The widest a compact-layout dialog gets (outer size).
const DIALOG_MAX_W: f32 = 560.0;

/// A dialog's content width: `desktop`, or in the compact layout what fits across the screen
/// inside a frame with `margin_x` of inner margin on each side.
pub fn dialog_width(ctx: &egui::Context, desktop: f32, margin_x: f32) -> f32 {
    if !is_compact(ctx) {
        return desktop;
    }
    desktop.min((ctx.content_rect().width().min(DIALOG_MAX_W + 2.0 * DIALOG_INSET) - 2.0 * (DIALOG_INSET + margin_x)).max(120.0))
}

/// In the compact layout a dialog window spans the screen (inside the safe area) at a fixed width
/// and scrolls when it is taller than the screen; elsewhere `win` is returned unchanged (desktop
/// dialogs fit the window on their own, #781).
pub fn dialog<'a>(ctx: &egui::Context, win: egui::Window<'a>) -> egui::Window<'a> {
    if !is_compact(ctx) {
        return win;
    }
    let area = ctx.content_rect().shrink(DIALOG_INSET);
    // egui's window sizes are its outer size (frame margins included); a phone in landscape gets
    // a dialog of readable width, not one stretched across the screen
    let w = area.width().min(DIALOG_MAX_W);
    // auto-sized: as tall as its contents up to the screen, then it scrolls
    win.auto_sized().min_width(w).max_width(w).max_height(area.height()).vscroll(true).constrain_to(area)
}

/// Where a full sheet goes: the width of the screen, from just under the status bar to the
/// bottom edge (under the home indicator too; [`sheet_frame`] keeps its content above it).
pub fn sheet_rect(ctx: &egui::Context) -> Rect {
    let screen = ctx.viewport_rect();
    let safe = ctx.content_rect();
    // the soft keyboard up: the sheet ends on it, so its fields and confirm stay in view
    let bottom = if keyboard_up(ctx) { safe.bottom() } else { screen.bottom() };
    Rect::from_min_max(egui::pos2(screen.left(), safe.top() + 8.0), egui::pos2(screen.right(), bottom))
}

/// The soft keyboard takes the bottom of the screen: far more than a home indicator's inset.
fn keyboard_up(ctx: &egui::Context) -> bool {
    ctx.viewport_rect().bottom() - ctx.content_rect().bottom() > 80.0
}

/// A full sheet's frame: rounded at the top, square where it meets the bottom edge, its content
/// clear of the home indicator.
pub fn sheet_frame(ctx: &egui::Context) -> egui::Frame {
    // (the home indicator's inset; with the keyboard up the sheet ends on the keyboard instead)
    let inset = if keyboard_up(ctx) { 0.0 } else { (ctx.viewport_rect().bottom() - ctx.content_rect().bottom()).max(0.0) };
    egui::Frame::window(&ctx.global_style()).corner_radius(egui::CornerRadius { nw: 14, ne: 14, sw: 0, se: 0 }).inner_margin(egui::Margin {
        left: 16,
        right: 16,
        top: 6,
        bottom: (12.0 + inset).round() as i8,
    })
}

/// How long a sheet takes to slide up (seconds).
const SHEET_SLIDE: f64 = 0.28;

/// `win` as a full sheet ([`sheet_rect`]) that slides up from the bottom edge when it is first
/// shown, then stays: its pages change inside it. `id`: the window's, to time the slide by.
pub fn sheet_window<'a>(ctx: &egui::Context, win: egui::Window<'a>, id: egui::Id) -> egui::Window<'a> {
    let full = sheet_rect(ctx);
    let key = id.with("sheet-shown");
    let pass = ctx.cumulative_pass_nr();
    let now = ctx.input(|i| i.time);
    // (shown again after a pass without it: the slide starts over)
    let start = match ctx.data(|d| d.get_temp::<(f64, u64)>(key)) {
        Some((start, last)) if last + 1 >= pass => start,
        _ => now,
    };
    ctx.data_mut(|d| d.insert_temp(key, (start, pass)));
    let t = ((now - start) / SHEET_SLIDE).clamp(0.0, 1.0) as f32;
    if t < 1.0 {
        ctx.request_repaint();
    }
    // ease out: fast at first, settling into place
    let shown = 1.0 - (1.0 - t).powi(3);
    let rect = full.translate(egui::vec2(0.0, (1.0 - shown) * full.height()));
    win.fixed_rect(rect).constrain(false).vscroll(false)
}

/// Finger-sized controls for a dialog in the compact layout: 44 pt buttons and fields, 16 pt
/// text, larger check boxes and radio buttons, and more room between rows. Nothing elsewhere.
pub fn finger_style(ui: &mut egui::Ui) {
    if !is_compact(ui.ctx()) {
        return;
    }
    let s = ui.style_mut();
    s.spacing.interact_size.y = 44.0;
    s.spacing.button_padding = vec2(16.0, 11.0);
    s.spacing.item_spacing = vec2(10.0, 12.0);
    s.spacing.icon_width = 24.0;
    s.spacing.icon_width_inner = 14.0;
    s.spacing.icon_spacing = 10.0;
    for style in [egui::TextStyle::Body, egui::TextStyle::Button, egui::TextStyle::Monospace] {
        if let Some(f) = s.text_styles.get_mut(&style) {
            f.size = f.size.max(16.0);
        }
    }
}

/// A labelled row for dialogs: in the compact layout the label sits above its controls, which
/// wrap; elsewhere `add` comes back for the caller's usual fixed label column.
pub fn stacked_row<R, F: FnOnce(&mut egui::Ui) -> R>(ui: &mut egui::Ui, label: egui::RichText, add: F) -> Result<R, F> {
    if !is_compact(ui.ctx()) {
        return Err(add);
    }
    Ok(ui
        .vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            // (no blank line for a row without a label)
            if !label.text().is_empty() {
                ui.label(label);
            }
            ui.horizontal_wrapped(add).inner
        })
        .inner)
}
