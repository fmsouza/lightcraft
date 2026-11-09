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

pub use display::display_profile;
pub(super) use display::view_options;
use display::{display_tab, interface_tab};
use general::general_tab;
use import::import_tab;
pub(crate) use libraries::{can_switch_library, free_name};
pub use libraries::{forget_library, library_name, new_library, open_library, open_library_name, recent_libraries, remember_library};
use performance::performance_tab;
use slideshow::slideshow_tab;
pub(crate) use widgets::{check, choices, heading, hint, row};

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
fn phone_sections(ui: &mut egui::Ui, tab: &mut String) -> bool {
    let t = Tokens::get(ui.ctx());
    let key = egui::Id::unique("settings-phone-page");
    let pass = ui.ctx().cumulative_pass_nr();
    let (mut open, last) = ui.data(|d| d.get_temp::<(bool, u64)>(key)).unwrap_or((false, 0));
    if last + 1 < pass {
        // shown again: a plain Settings (General) starts on the list; one opened on a section
        // (AI Denoise from its panel, Faces …) goes straight to its page
        open = tab != "general";
    }
    let row = |ui: &mut egui::Ui, id: String, label: &str, back: bool| -> bool {
        let w = ui.available_width();
        let (r, resp) = ui.allocate_exact_size(egui::vec2(w, 48.0), egui::Sense::click());
        register(ui, id, r);
        resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, label));
        let p = ui.painter();
        if resp.is_pointer_button_down_on() {
            p.rect_filled(r, 8.0, t.hover);
        }
        let (icon, x) = if back { (crate::icons::Icon::ChevronLeft, r.left() + 12.0) } else { (crate::icons::Icon::ChevronRight, r.right() - 12.0) };
        crate::icons::paint(p, egui::Rect::from_center_size(egui::pos2(x, r.center().y), egui::vec2(16.0, 16.0)), icon, t.text_dim);
        let text_x = if back { r.left() + 30.0 } else { r.left() + 8.0 };
        p.text(egui::pos2(text_x, r.center().y), egui::Align2::LEFT_CENTER, label, t.font(16.0), if back { t.accent } else { t.text });
        resp.clicked()
    };
    let mut list = !open;
    if open {
        if row(ui, "button:settingsBack".into(), crate::i18n::tr("All Settings"), true) {
            open = false;
            list = true;
        } else if let Some((_, label)) = TABS.iter().find(|(id, _)| id == tab) {
            ui.label(egui::RichText::new(crate::i18n::tr(label)).font(t.semibold(18.0)).color(t.text));
        }
    }
    if list {
        for (id, label) in TABS {
            if row(ui, format!("button:settingsTab-{id}"), crate::i18n::tr(label), false) {
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
    ui.set_width((ui.ctx().content_rect().width() - 64.0).clamp(240.0, PAGE_SIZE.x));
    let top = ui.cursor().top();
    // The page drawn is the one that was open when the frame began: a tab clicked now opens on
    // the next frame, so a text box on the page being left is drawn once more and saves what was
    // typed in it (the click took the focus from it).
    let shown = tab.clone();
    let portrait_phone = crate::panels::compact::is_compact(ui.ctx()) && ui.ctx().content_rect().width() < crate::panels::compact::COMPACT_WIDTH;
    if portrait_phone {
        // a phone held upright: the sections as a list, a section's page with ‹ back to it (held
        // sideways it has the width for the tab bar)
        if phone_sections(ui, tab) {
            return;
        }
    } else {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(4.0, 4.0);
            for (id, label) in TABS {
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
    if shown == "slideshow" {
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
