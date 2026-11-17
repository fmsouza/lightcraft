//! The system's Back (Android's back button or gesture): one step back through what is on screen,
//! as a phone app goes: a menu or sheet closes, a pushed page goes back, a dialog closes, the
//! search, the drawer or Select mode ends, then the editor, a tool or a view goes back toward the
//! photos. Nothing left to go back from: the host leaves the app.

use std::sync::atomic::{AtomicBool, Ordering};

use serde_json::json;

use crate::LightcraftApp;
use crate::state::{Dialog, ViewMode};

/// Back was pressed with nowhere left to go: the host leaves the app ([`take_leave`]).
static LEAVE: AtomicBool = AtomicBool::new(false);

/// The system's Back key (Android's button or gesture, which winit hands over as the browser's
/// Back key): a step back, or, with nowhere to go, a request to leave.
pub(crate) fn on_back_key(app: &mut LightcraftApp, ctx: &egui::Context) {
    if ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE, egui::Key::BrowserBack)) && !system_back(app, ctx) {
        LEAVE.store(true, Ordering::Relaxed);
    }
}

/// Whether Back asked to leave the app since the last call (the host sends it to the background).
pub fn take_leave() -> bool {
    LEAVE.swap(false, Ordering::Relaxed)
}

/// Go one step back; false when there is nothing to go back from (the photos, as they are).
pub fn system_back(app: &mut LightcraftApp, ctx: &egui::Context) -> bool {
    if egui::Popup::is_any_open(ctx) {
        egui::Popup::close_all(ctx);
        return true;
    }
    if close_a_sheet(ctx) {
        return true;
    }
    if let Some(dlg) = &app.ui.dialog {
        let page_back = match dlg {
            Dialog::Settings { .. } => crate::panels::settings::phone_back(ctx),
            Dialog::About => crate::dialogs::about_phone::back(ctx),
            _ => false,
        };
        if !page_back {
            if matches!(dlg, Dialog::Import { .. }) {
                app.renderer.forget_imports();
            }
            app.ui.dialog = None;
        }
        return true;
    }
    if super::compact::close_search(ctx) {
        return true;
    }
    if app.ui.left_panel && super::compact::is_compact(ctx) {
        let _ = app.run("view.leftPanel", json!({}));
        return true;
    }
    if app.ui.select_mode {
        let _ = app.run("view.selectMode", json!({"on": false}));
        return true;
    }
    let grid = matches!(app.ui.view, ViewMode::PhotoGrid | ViewMode::SquareGrid);
    let elsewhere =
        app.ui.lights_out != crate::state::LightsOut::On || app.ui.keyword_painter.is_some() || app.ui.fullscreen || !app.ui.tool.is_empty() || !grid;
    if !elsewhere {
        return false;
    }
    let view = app.ui.view;
    let _ = app.run("view.back", json!({}));
    // (a view `view.back` leaves alone, such as People or the map: the photos)
    if app.ui.view == view && !grid && view != ViewMode::Detail && app.ui.dialog.is_none() {
        app.ui.view = ViewMode::PhotoGrid;
    }
    true
}

/// Close the bottom sheet that is up, if one is.
fn close_a_sheet(ctx: &egui::Context) -> bool {
    use super::{filter_sheet, more_sheet, rate_sheet, share_sheet, sort_sheet};
    let sheets: [(fn(&egui::Context) -> bool, fn(&egui::Context)); 5] = [
        (more_sheet::is_open, more_sheet::close),
        (filter_sheet::is_open, filter_sheet::close),
        (rate_sheet::is_open, rate_sheet::close),
        (sort_sheet::is_open, sort_sheet::close),
        (share_sheet::is_open, share_sheet::close),
    ];
    for (open, close) in sheets {
        if open(ctx) {
            close(ctx);
            return true;
        }
    }
    false
}
