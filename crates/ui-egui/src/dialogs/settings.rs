//! The Settings dialog (⌘,): General, Import, Performance, Display, Interface, Slideshow, Faces,
//! AI Denoise.
//!
//! Changes apply immediately (no OK/Cancel). Where they are stored:
//! - **app settings** ([`crate::state::AppSettings`]: startup view, delete confirmation, GPU,
//!   preview size, filmstrip/grid badges, last library, monitor profile, whether Import Lightroom
//!   Catalog… asks where a catalog goes) and Auto Advance live in
//!   the UI state,
//!   saved by the host in its config folder (`ui.json`);
//! - **library settings** (import defaults, XMP sidecars, thumbnail cache size, slideshow
//!   playback) go through the `library.preferences` / `library.xmpPreferences` /
//!   `slideshow.playback` commands into the library's `prefs.json`,
//!   so they travel with the library.

use egui::RichText;
use serde_json::{Value, json};

use crate::LightcraftApp;
use crate::state::{GridBadges, PREVIEW_LIMITS, StartupView};
use crate::theme::Tokens;
use crate::widgets::register;

mod appearance;
mod display;
mod general;
mod import;
mod libraries;
mod performance;
mod slideshow;
mod widgets;

/// Whether Settings is shown in a phone or tablet app (`Services::mobile`): settings that only mean
/// something on a computer (folders, other programs, monitors, GPU and memory) are left out.
pub(super) fn mobile(app: &LightcraftApp) -> bool {
    app.services.mobile
}

/// The tabs this host shows: a phone or tablet app has no Display tab (its monitor profile is the
/// system's colour management).
fn tabs(app: &LightcraftApp) -> Vec<(&'static str, &'static str)> {
    // (a phone or tablet manages its display's colour itself, and has no AI Denoise)
    TABS.iter().copied().filter(|(id, _)| !(mobile(app) && matches!(*id, "display" | "denoise"))).collect()
}

pub use display::display_profile;
pub(super) use display::view_options;
use display::{display_tab, interface_tab};
use general::general_tab;
use import::import_tab;
pub(crate) use libraries::{can_switch_library, free_name};
pub use libraries::{forget_library, library_name, new_library, open_library, open_library_name, recent_libraries, remember_library};
use performance::performance_tab;
use slideshow::slideshow_tab;
pub(crate) use widgets::{PhoneButton, check, choices, heading, hint, open_subpage, phone_button, phone_slider, row, tick_box};

pub mod slide_layout;
pub mod slide_overlays;
pub mod slide_templates;
pub mod slide_titles;

/// (id, label) of the tabs, in order.
pub const TABS: &[(&str, &str)] = &[
    ("general", "General"),
    ("import", "Import"),
    ("performance", "Performance"),
    ("display", "Display"),
    ("interface", "Interface"),
    ("slideshow", "Slideshow"),
    ("faces", "Faces"),
    ("denoise", "AI Denoise"),
];

/// Thumbnail cache sizes offered (MB).
const CACHE_SIZES: [u32; 5] = [512, 1024, 2048, 4096, 8192];

const LABEL_W: f32 = 150.0;

/// The page of `tab` (the tab bar in [`window`] switches `tab`).
pub fn body(app: &mut LightcraftApp, ui: &mut egui::Ui, tab: &str) {
    let t = Tokens::get(ui.ctx());
    match tab {
        "import" => import_tab(app, ui, &t),
        "performance" => performance_tab(app, ui, &t),
        // (a phone or tablet app has no Display tab: a link to it shows General)
        "display" | "denoise" if mobile(app) => general_tab(app, ui, &t),
        "display" => display_tab(app, ui, &t),
        "interface" => interface_tab(app, ui, &t),
        "slideshow" => slideshow_tab(app, ui, &t, true),
        "faces" => crate::panels::faces::settings_tab(app, ui, &t),
        "denoise" => crate::panels::denoise::settings_tab(app, ui, &t),
        _ => general_tab(app, ui, &t),
    }
}

/// The widest the Settings window's content gets, and the tallest its pages get: the window is
/// one size on every tab, so it stays where it is as the tabs are gone through.
const PAGE_SIZE: egui::Vec2 = egui::vec2(560.0, 640.0);

/// The least room in which a page scrolls by itself. In a window with less (far below the
/// desktop minimum), the tabs and the page scroll together in the dialog's own scroll area.
const PAGE_MIN_HEIGHT: f32 = 120.0;

/// The share of the page's room the pinned slide preview of the Slideshow tab takes, when the page
/// has more room than it needs to keep [`PAGE_MIN_HEIGHT`] for itself.
const PINNED_PREVIEW_SHARE: f32 = 0.4;

/// What the preview's heading, the one-line hint under the slide, its Photo / Intro / Ending
/// choice and the rule under it take around the slide (at the default spacing, about 82 pt).
const PREVIEW_CHROME: f32 = 82.0;

/// The phone layout's Settings: the sections as finger-sized rows (true: that list is the page,
/// nothing more to draw), or above a section's page, ‹ back to them and its name. A plain Settings
/// opens on the list; one opened on another section opens that section's page.
/// A phone held upright: Settings fills the screen with a header of its own (‹ back to the list
/// of sections, the page's name, ✕), as a phone's settings do; the dialog's title bar and Close
/// button are left out ([`super::super::dialogs`]).
pub(crate) fn portrait_phone(ctx: &egui::Context) -> bool {
    crate::panels::compact::is_compact(ctx) && ctx.content_rect().width() < crate::panels::compact::COMPACT_WIDTH
}

/// Set for a frame when a sheet header's ✕ is tapped; the dialog takes it and closes.
pub(crate) fn close_requested(ctx: &egui::Context) -> bool {
    ctx.data_mut(|d| d.remove_temp::<bool>(egui::Id::unique("settings-phone-close"))).unwrap_or(false)
}

/// The system's Back in a phone's Settings: a pushed page goes, then a section's page gives way to
/// the list of sections. False: on the list already (Back closes Settings).
pub(crate) fn phone_back(ctx: &egui::Context) -> bool {
    if widgets::subpage(ctx).is_some() {
        widgets::close_subpage(ctx);
        return true;
    }
    let key = egui::Id::unique("settings-phone-page");
    match ctx.data(|d| d.get_temp::<(bool, u64)>(key)) {
        Some((true, pass)) => {
            ctx.data_mut(|d| d.insert_temp(key, (false, pass)));
            true
        }
        _ => false,
    }
}

/// A phone sheet's header: ‹ (`back`: its widget id, `None`: no ‹) at the left, `title` in the
/// middle, ✕ at the right; true when ‹ is tapped. ✕ asks the sheet to close ([`close_requested`]).
pub(crate) fn sheet_header(ui: &mut egui::Ui, title: &str, back: Option<&str>, close: &str) -> bool {
    let t = Tokens::get(ui.ctx());
    let w = ui.available_width();
    let (bar, _) = ui.allocate_exact_size(egui::vec2(w, 48.0), egui::Sense::hover());
    let icon_button = |ui: &mut egui::Ui, id: &str, x: f32, icon: crate::icons::Icon, label: &str| -> bool {
        let r = egui::Rect::from_center_size(egui::pos2(x, bar.center().y), egui::vec2(44.0, 44.0));
        let resp = ui.interact(r, egui::Id::unique(id), egui::Sense::click());
        register(ui, id, r);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
        if resp.is_pointer_button_down_on() {
            ui.painter().circle_filled(r.center(), 20.0, t.hover);
        }
        crate::icons::paint(ui.painter(), egui::Rect::from_center_size(r.center(), egui::vec2(20.0, 20.0)), icon, t.text);
        resp.clicked()
    };
    ui.painter().text(bar.center(), egui::Align2::CENTER_CENTER, title, t.semibold(17.0), t.text);
    let back = back.is_some_and(|id| icon_button(ui, id, bar.left() + 18.0, crate::icons::Icon::ChevronLeft, crate::i18n::tr("Back")));
    if icon_button(ui, close, bar.right() - 18.0, crate::icons::Icon::Close, crate::i18n::tr("Close")) {
        ui.data_mut(|d| d.insert_temp(egui::Id::unique("settings-phone-close"), true));
    }
    back
}

/// A page a section pushed of its own ([`widgets::open_subpage`]): a face model's. False: a list.
pub(crate) fn own_page(app: &mut LightcraftApp, ui: &mut egui::Ui, id: &str) -> bool {
    match id.strip_prefix("faceModel:") {
        Some(model) => {
            super::faces::phone_model_page(app, ui, model);
            true
        }
        None => false,
    }
}

/// The header and, on the list, the sections as rows; true while the list is what shows (no page).
fn phone_sections(ui: &mut egui::Ui, tab: &mut String, tabs: &[(&'static str, &'static str)]) -> bool {
    let t = Tokens::get(ui.ctx());
    let key = egui::Id::unique("settings-phone-page");
    let pass = ui.ctx().cumulative_pass_nr();
    let (mut open, last) = ui.data(|d| d.get_temp::<(bool, u64)>(key)).unwrap_or((false, 0));
    if last + 1 < pass {
        // shown again: a plain Settings (General) starts on the list; one opened on a section
        // (AI Denoise from its panel, Faces …) goes straight to its page
        open = tab != "general";
        widgets::close_subpage(ui.ctx());
    }
    let sub = widgets::subpage(ui.ctx());
    // (a list a row opened is a page under the section's: its name, ‹ back to the section)
    let title = match &sub {
        Some(page) if open => page.title.clone(),
        _ => tabs.iter().find(|(id, _)| open && id == tab).map_or(crate::i18n::tr("Settings"), |(_, label)| crate::i18n::tr(label)).to_string(),
    };
    let mut list = !open;
    if sheet_header(ui, &title, open.then_some("button:settingsBack"), "button:settingsClose") {
        if sub.is_some() {
            widgets::close_subpage(ui.ctx());
        } else {
            open = false;
            list = true;
        }
    }
    let w = ui.available_width();
    if list {
        ui.separator();
        for (id, label) in tabs {
            let (r, resp) = ui.allocate_exact_size(egui::vec2(w, 48.0), egui::Sense::click());
            register(ui, format!("button:settingsTab-{id}"), r);
            let label = crate::i18n::tr(label);
            resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
            let p = ui.painter();
            if resp.is_pointer_button_down_on() {
                p.rect_filled(r, 8.0, t.hover);
            }
            let chevron = egui::Rect::from_center_size(egui::pos2(r.right() - 12.0, r.center().y), egui::vec2(16.0, 16.0));
            crate::icons::paint(p, chevron, crate::icons::Icon::ChevronRight, t.text_dim);
            p.text(egui::pos2(r.left() + 8.0, r.center().y), egui::Align2::LEFT_CENTER, label, t.font(16.0), t.text);
            if resp.clicked() {
                *tab = id.to_string();
                open = true;
            }
        }
        ui.ctx().request_repaint();
    }
    ui.data_mut(|d| d.insert_temp(key, (open, pass)));
    list && !open
}

/// The Settings window's content: the tab bar, which stays put, above the page of `tab`
/// ([`body`]), which scrolls when it is taller than the room for it, with a scroll handle that
/// shows without the pointer on it. `max_height` is the room the app window leaves for both: the
/// two together never take more, so the scroll area every dialog is drawn in has nothing to
/// scroll (but for a window with less than [`PAGE_MIN_HEIGHT`] for the page, where that area
/// scrolls the two together). Every page gets the same room whatever it holds (but for the pinned
/// preview of Slideshow), and each keeps a scroll position of its own. Nothing here is wider than the window from 640 pt of app window
/// up: a row's controls wrap ([`row`]) and paths are shortened ([`path_label`]).
/// On Slideshow the slide preview is pinned between the tab bar and the page (where the room
/// allows), and comes off that page's room.
pub fn window(app: &mut LightcraftApp, ui: &mut egui::Ui, tab: &mut String, max_height: f32) {
    // (a phone's sheet is the width of the screen)
    if portrait_phone(ui.ctx()) {
        ui.set_width(ui.available_width());
    } else {
        ui.set_width((ui.ctx().content_rect().width() - 64.0).clamp(240.0, PAGE_SIZE.x));
    }
    let top = ui.cursor().top();
    // The page drawn is the one that was open when the frame began: a tab clicked now opens on
    // the next frame, so a text box on the page being left is drawn once more and saves what was
    // typed in it (the click took the focus from it).
    let tabs = tabs(app);
    let shown = tab.clone();
    if portrait_phone(ui.ctx()) {
        // a phone held upright: the sections as a list, a section's page with ‹ back to it (held
        // sideways it has the width for the tab bar)
        if phone_sections(ui, tab, &tabs) {
            return;
        }
        // a list a row opened, in place of the section's page
        if let Some(page) = widgets::subpage(ui.ctx()) {
            ui.separator();
            egui::ScrollArea::vertical().id_salt(("settings-subpage", page.id.as_str())).max_height(max_height).auto_shrink([false, false]).show(
                ui,
                |ui| {
                    if !own_page(app, ui, &page.id) {
                        widgets::subpage_list(ui, &page);
                    }
                    register(ui, "settings:subpage", ui.min_rect());
                },
            );
            return;
        }
    } else {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
            for (id, label) in &tabs {
                if crate::widgets::text_button(ui, &format!("settingsTab-{id}"), label, shown == *id).clicked() {
                    *tab = id.to_string();
                    ui.ctx().request_repaint();
                }
            }
        });
    }
    ui.separator();
    // (a point less than the room left, so rounding never makes the dialog's own scroll area scroll)
    let room = max_height - (ui.cursor().top() - top) - 1.0;
    if room < PAGE_MIN_HEIGHT {
        body(app, ui, shown.as_str());
        register(ui, "settings:page", ui.min_rect());
        return;
    }
    let mut height = room.min(PAGE_SIZE.y);
    // The slide preview stays above the page on Slideshow: hovering a template in the Template
    // Browser previews it there, so it has to stay in view however far down the templates are
    // scrolled to. With its heading and choice it takes [`PINNED_PREVIEW_SHARE`] of the room
    // (at least the least tall slide); where that leaves less than [`PAGE_MIN_HEIGHT`] for the
    // page, it scrolls with the page instead (as in the short window above).
    let mut pinned = false;
    if shown == "slideshow" && !mobile(app) {
        let slide_h = (height * PINNED_PREVIEW_SHARE - PREVIEW_CHROME).max(crate::panels::slide_layout::PREVIEW_MIN_H);
        if height - (slide_h + PREVIEW_CHROME) >= PAGE_MIN_HEIGHT {
            let before = ui.cursor().top();
            crate::panels::slide_layout::preview(app, ui, &Tokens::get(ui.ctx()), slide_h);
            ui.add_space(2.0);
            ui.separator();
            height = (height - (ui.cursor().top() - before)).max(PAGE_MIN_HEIGHT);
            pinned = true;
        }
    }
    // A page that goes on below shows its scroll handle all the time: without it, a page a line
    // or two too long looks cut off, not scrollable.
    ui.spacing_mut().scroll.dormant_handle_opacity = 0.6;
    let page = egui::ScrollArea::vertical()
        .id_salt(("settings-page", shown.as_str()))
        .max_height(height)
        .min_scrolled_height(height)
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add_space(2.0);
            if pinned {
                slideshow_tab(app, ui, &Tokens::get(ui.ctx()), false);
            } else {
                body(app, ui, shown.as_str());
            }
            // (what the page took: wider than the room for it would mean a control is cut off)
            register(ui, "settings:page", ui.min_rect());
        });
    register(ui, "settings:pageRoom", page.inner_rect);
}
