//! Headless tests of the compact (phone-width) layout: the photo keeps its room, the right panel
//! and Presets open in a full-width bottom sheet under it, My Photos is a full-width drawer, and a
//! desktop-sized window is unchanged.

use std::time::Duration;

use serde_json::json;

use crate::headless::Headless;
use crate::state::{RightPanel, ViewMode};
use crate::{LightcraftApp, Services};

const T: Duration = Duration::from_secs(20);
const SETTLE: Duration = Duration::from_secs(120);
const PHONE: [f32; 2] = [402.0, 874.0];

fn demo(size: [f32; 2], ui: serde_json::Value) -> Headless {
    let services = Services { png: None, ..Default::default() };
    let app = LightcraftApp::new(lightcraft_engine::Session::with_demo(), services);
    let mut h = Headless::new(app, size, 1.0);
    let r = h.request("ui.set", ui, T);
    assert_eq!(r["ok"], true, "{r}");
    h.settle(SETTLE);
    h
}

fn widget(h: &Headless, id: &str) -> Option<egui::Rect> {
    h.app.widgets.iter().find(|(w, _)| w == id).map(|(_, r)| *r)
}

fn click(h: &mut Headless, id: &str) {
    let r = h.request("ui.clickWidget", json!({"id": id}), T);
    assert_eq!(r["ok"], true, "{r}");
    h.step();
    h.step();
}

#[test]
fn phone_width_puts_the_editor_in_a_sheet_under_the_photo() {
    let mut h = demo(PHONE, json!({"view": "detail", "right": "edit", "leftPanel": true}));
    assert!(crate::panels::compact::is_compact(&h.view.ctx));
    // entering the compact layout closed My Photos, which would cover the photo
    assert!(!h.app.ui.left_panel);
    let sheet = widget(&h, "panel:sheet").expect("the edit sheet is up");
    let right = widget(&h, "panel:right_panel").expect("the Edit panel is in the sheet");
    assert!(right.width() >= PHONE[0] - 2.0, "the Edit panel spans the width: {right:?}");
    assert!(sheet.top() > 300.0 && sheet.bottom() <= PHONE[1], "{sheet:?}");
    let canvas = h.app.canvas_rect.expect("the loupe is drawn");
    assert!(canvas.width() >= PHONE[0] - 2.0 && canvas.height() > 250.0, "the photo keeps its room: {canvas:?}");
    assert!(canvas.bottom() <= sheet.top() + 1.0, "the photo sits above the sheet");
    assert!(widget(&h, "slider:light.exposure").is_some(), "the Light sliders are in the sheet");
    // a tool in the sheet's row switches the panel; Presets replaces it at full width
    click(&mut h, "icon:crop");
    assert_eq!(h.app.ui.right, RightPanel::Crop);
    click(&mut h, "icon:presets");
    assert!(h.app.ui.presets);
    let presets = widget(&h, "panel:sheet").expect("still in the sheet");
    assert!(presets.width() >= PHONE[0] - 2.0);
    // Hide puts the sheet away and gives the photo the height back
    click(&mut h, "icon:sheetClose");
    assert!(!h.app.ui.presets && h.app.ui.right == RightPanel::None);
    h.step();
    assert!(widget(&h, "panel:sheet").is_none());
    assert!(h.app.canvas_rect.is_some_and(|c| c.height() > canvas.height() + 150.0));
    // a section in the tool row brings it back on the Edit panel, that section alone
    click(&mut h, "icon:light");
    assert_eq!(h.app.ui.right, RightPanel::Edit);
    assert!(widget(&h, "panel:sheet").is_some());
    assert!(widget(&h, "slider:light.exposure").is_some(), "Light's sliders");
    assert!(widget(&h, "slider:color.saturation").is_none(), "only Light's");
    // pressed again, it puts the sheet away
    click(&mut h, "icon:light");
    assert_eq!(h.app.ui.right, RightPanel::None);
}

#[test]
fn phone_width_opens_my_photos_as_a_drawer() {
    let mut h = demo(PHONE, json!({"view": "photoGrid"}));
    click(&mut h, "icon:sidebar");
    assert!(h.app.ui.left_panel);
    let left = widget(&h, "panel:left_panel").expect("My Photos is open");
    assert!(left.width() >= PHONE[0] - 2.0, "full width: {left:?}");
    // picking a source closes it onto that source's photos
    click(&mut h, "source:picks");
    assert_eq!(h.app.session.source, lightcraft_engine::LibrarySource::Picks);
    assert!(!h.app.ui.left_panel);
}

#[test]
fn desktop_width_keeps_the_side_panels() {
    let h = demo([1400.0, 900.0], json!({"view": "detail", "right": "edit"}));
    assert!(!crate::panels::compact::is_compact(&h.view.ctx));
    assert!(widget(&h, "panel:sheet").is_none());
    let right = widget(&h, "panel:right_panel").expect("the right panel");
    assert!(right.width() < 600.0 && right.right() > 1300.0, "a column at the right: {right:?}");
    assert!(widget(&h, "icon:discord").is_some(), "the desktop top bar");
}

/// The dialog window's rectangle (`panels::dialogs` gives every dialog this id).
/// The open dialog's window (whichever window id it has: Settings and the import review keep
/// their own).
fn dialog_rect(h: &Headless) -> Option<egui::Rect> {
    h.app.widgets.iter().find(|(w, _)| w == "dialog:window").map(|(_, r)| *r)
}

fn open(h: &mut Headless, command: &str) {
    let r = h.request("engine.execute", json!({"command": command, "params": {}}), T);
    assert_eq!(r["ok"], true, "{command}: {r}");
    for _ in 0..4 {
        h.step();
    }
}

#[test]
fn phone_width_dialogs_fit_the_screen() {
    let mut h = demo(PHONE, json!({"view": "detail"}));
    let r = h.request("engine.execute", json!({"command": "library.selectAll", "params": {}}), T);
    assert_eq!(r["ok"], true, "{r}");
    // the wide ones on the desktop: Settings (560 pt), Export (label column + button rows),
    // Shortcuts (a three-column table), Merge (a 520 pt preview beside its options), and two
    // ordinary ones
    for command in ["app.settings", "dialog.export", "app.shortcuts", "dialog.mergeHdr", "dialog.labelNames", "dialog.copySettings"] {
        open(&mut h, command);
        assert!(h.app.ui.dialog.is_some(), "{command} opened a dialog");
        // (most are sheets that slide up on a phone: measured once in place)
        for _ in 0..60 {
            h.step();
        }
        let r = dialog_rect(&h).unwrap_or_else(|| panic!("{command}: no dialog window"));
        assert!(r.left() >= 0.0 && r.right() <= PHONE[0], "{command} fits across: {r:?}");
        assert!(r.top() >= 0.0 && r.bottom() <= PHONE[1], "{command} fits down: {r:?}");
        assert!(r.width() > PHONE[0] - 40.0, "{command} uses the width: {r:?}");
        h.request("ui.dialog.cancel", json!({}), T);
        h.step();
    }
    h.app.ui.dialog = Some(crate::state::Dialog::ConfirmDelete { count: 1 });
    for _ in 0..4 {
        h.step();
    }
    let r = dialog_rect(&h).expect("the delete confirmation");
    assert!(r.left() >= 0.0 && r.right() <= PHONE[0], "{r:?}");
}

#[test]
fn desktop_width_dialogs_keep_their_size() {
    let mut h = demo([1400.0, 900.0], json!({"view": "detail"}));
    open(&mut h, "app.settings");
    let r = dialog_rect(&h).expect("Settings");
    assert!(r.width() >= 560.0 && r.width() < 900.0, "Settings at its desktop width: {r:?}");
    // (in a fresh session: dialogs share one window, which keeps the last one's width)
    let mut h = demo([1400.0, 900.0], json!({"view": "detail"}));
    open(&mut h, "dialog.labelNames");
    let r = dialog_rect(&h).expect("label names");
    assert!(r.width() < 600.0, "an ordinary dialog stays compact on the desktop: {r:?}");
}

fn exposure(h: &Headless) -> f64 {
    let id = h.app.session.active().expect("a photo in Detail");
    h.app.session.develop_of(id).unwrap_or_default().light.exposure
}

/// The edit sheet scrolled (mouse wheel) so the Light sliders are clear of the bottom bar; the
/// Exposure track's rectangle.
fn exposure_in_view(h: &mut Headless) -> egui::Rect {
    let sheet = widget(h, "panel:sheet").expect("the edit sheet");
    let r = h.request("ui.move", json!({"x": sheet.center().x, "y": sheet.center().y}), T);
    assert_eq!(r["ok"], true, "{r}");
    let r = h.request("ui.scroll", json!({"dy": -220.0}), T);
    assert_eq!(r["ok"], true, "{r}");
    for _ in 0..30 {
        h.step();
    }
    let track = widget(h, "slider:light.exposure").expect("the Exposure slider");
    assert!(track.bottom() < sheet.bottom() - 60.0, "Exposure scrolled into view: {track:?} in {sheet:?}");
    track
}

/// A finger touches the screen (no press: nothing is clicked), so the layout takes its touch-screen
/// sizes before a test measures where things are.
fn touch_screen(h: &mut Headless) {
    let p = egui::pos2(1.0, 1.0);
    let touch = |phase| egui::Event::Touch { device_id: egui::TouchDeviceId(1), id: egui::TouchId(9), phase, pos: p, force: None };
    h.input([touch(egui::TouchPhase::Start), touch(egui::TouchPhase::End)]);
    for _ in 0..3 {
        h.step();
    }
}

/// One finger from `from` to `to` in `steps` moves, then lifted.
fn touch_drag(h: &mut Headless, from: egui::Pos2, to: egui::Pos2, steps: usize) {
    let touch = |phase, pos| egui::Event::Touch { device_id: egui::TouchDeviceId(1), id: egui::TouchId(1), phase, pos, force: None };
    let button = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    h.input([touch(egui::TouchPhase::Start, from), egui::Event::PointerMoved(from), button(from, true)]);
    h.step();
    for i in 1..=steps {
        let p = from + (to - from) * (i as f32 / steps as f32);
        h.input([touch(egui::TouchPhase::Move, p), egui::Event::PointerMoved(p)]);
        h.step();
    }
    h.input([touch(egui::TouchPhase::End, to), button(to, false), egui::Event::PointerGone]);
    h.step();
    h.step();
}

#[test]
fn touch_swipe_up_on_a_slider_scrolls_the_sheet_instead() {
    let mut h = demo(PHONE, json!({"view": "detail", "right": "edit"}));
    let before = exposure(&h);
    touch_screen(&mut h);
    let track = exposure_in_view(&mut h);
    // start on the track, left of the thumb, and swipe up
    let from = egui::pos2(track.left() + 20.0, track.center().y);
    touch_drag(&mut h, from, from - egui::vec2(4.0, 160.0), 16);
    assert_eq!(exposure(&h), before, "the value didn't move");
    let moved = widget(&h, "slider:light.exposure").expect("still on screen or scrolled");
    assert!(moved.top() < track.top() - 100.0, "the sheet scrolled: {track:?} → {moved:?}");
}

#[test]
fn touch_swipe_sideways_on_a_slider_adjusts_it_by_the_distance() {
    let mut h = demo(PHONE, json!({"view": "detail", "right": "edit"}));
    let before = exposure(&h);
    touch_screen(&mut h);
    let track = exposure_in_view(&mut h);
    // from near the left end, 40 pt right: by touch the value moves by 40 pt's worth, rather
    // than jumping to the finger (which would set it near the minimum)
    let from = egui::pos2(track.left() + 10.0, track.center().y);
    touch_drag(&mut h, from, from + egui::vec2(40.0, 2.0), 8);
    let after = exposure(&h);
    let spec = lightcraft_develop::controls::find("light.exposure").expect("exposure spec");
    let expected = 40.0 / track.width() as f64 * (spec.max - spec.min);
    assert!(after > before, "it went up: {before} → {after}");
    assert!((after - before - expected).abs() < expected * 0.4, "by about {expected}: {before} → {after}");
    // the slider stayed put: no scrolling
    assert_eq!(widget(&h, "slider:light.exposure").map(|r| r.top()), Some(track.top()));
}

#[test]
fn touch_tap_on_a_slider_leaves_the_value() {
    let mut h = demo(PHONE, json!({"view": "detail", "right": "edit"}));
    let before = exposure(&h);
    touch_screen(&mut h);
    let track = exposure_in_view(&mut h);
    let at = egui::pos2(track.left() + 10.0, track.center().y);
    touch_drag(&mut h, at, at, 0);
    assert_eq!(exposure(&h), before);
}

#[test]
fn mouse_click_on_a_slider_still_sets_the_value_there() {
    // no touch input in this session: the desktop behaviour
    let mut h = demo(PHONE, json!({"view": "detail", "right": "edit"}));
    let before = exposure(&h);
    let track = exposure_in_view(&mut h);
    let r = h.request("ui.click", json!({"x": track.left() + 2.0, "y": track.center().y}), T);
    assert_eq!(r["ok"], true, "{r}");
    for _ in 0..4 {
        h.step();
    }
    assert!(exposure(&h) < before - 1.0, "set near the minimum: {before} → {}", exposure(&h));
}

#[test]
fn mouse_click_on_a_slider_sets_the_value_on_a_touch_screen_too() {
    // the screen has been touched (as the mobile shells announce on their first frame), but the
    // click comes from a trackpad or mouse: it sets the value as on the desktop
    let mut h = demo(PHONE, json!({"view": "detail", "right": "edit"}));
    let before = exposure(&h);
    touch_screen(&mut h);
    let track = exposure_in_view(&mut h);
    let r = h.request("ui.click", json!({"x": track.left() + 2.0, "y": track.center().y}), T);
    assert_eq!(r["ok"], true, "{r}");
    for _ in 0..4 {
        h.step();
    }
    assert!(exposure(&h) < before - 1.0, "set near the minimum: {before} → {}", exposure(&h));
}

/// The soft keyboard, as a phone shell reports it: 330 pt of the screen's bottom.
const KEYBOARD: f32 = 330.0;

fn keyboard_up(h: &mut Headless) {
    h.set_safe_area(egui::SafeAreaInsets(egui::epaint::MarginF32 { bottom: KEYBOARD, ..Default::default() }));
    for _ in 0..8 {
        h.step();
    }
}

fn focus(h: &mut Headless, field: &str) {
    let r = h.request("ui.clickWidget", json!({"id": field}), T);
    assert_eq!(r["ok"], true, "{r}");
    h.step();
    h.step();
    assert!(h.view.ctx.memory(|m| m.focused()).is_some(), "{field} has focus");
}

#[test]
fn keyboard_lifts_the_layout_and_keeps_a_sheet_field_in_view() {
    let mut h = demo(PHONE, json!({"view": "detail", "right": "info"}));
    let sheet = widget(&h, "panel:sheet").expect("the Info sheet");
    // bring the first text field into the lower part of the sheet (a phone's finger-sized rows
    // put the photo's details above it)
    for _ in 0..20 {
        let title = widget(&h, "field:title").expect("the Title field");
        if title.bottom() <= sheet.bottom() - 4.0 {
            break;
        }
        let r = h.request("ui.move", json!({"x": sheet.center().x, "y": sheet.center().y}), T);
        assert_eq!(r["ok"], true, "{r}");
        let r = h.request("ui.scroll", json!({"dx": 0.0, "dy": -60.0}), T);
        assert_eq!(r["ok"], true, "{r}");
        h.step();
    }
    // the lowest text field visible in the sheet, which a 330 pt keyboard would cover
    let (field, rect) = h
        .app
        .widgets
        .iter()
        .filter(|(id, r)| id.starts_with("field:") && sheet.contains_rect(*r))
        .max_by(|a, b| a.1.bottom().total_cmp(&b.1.bottom()))
        .map(|(id, r)| (id.clone(), *r))
        .expect("a text field in the Info panel");
    assert!(rect.bottom() > PHONE[1] - KEYBOARD, "{field} starts where the keyboard goes: {rect:?}");
    focus(&mut h, &field);
    keyboard_up(&mut h);
    let content = h.view.ctx.content_rect();
    assert_eq!(content.bottom(), PHONE[1] - KEYBOARD, "the content area ends at the keyboard");
    let bar = widget(&h, "panel:tools").expect("the editor's tool row");
    assert!(bar.bottom() <= content.bottom() + 0.5, "the tool row sits on the keyboard: {bar:?}");
    let now = widget(&h, &field).expect("the field");
    assert!(
        now.bottom() <= content.bottom() && now.top() >= h.app.canvas_rect.map_or(0.0, |c| c.bottom()),
        "{field} in view above the keyboard: {now:?}"
    );
}

#[test]
fn keyboard_keeps_a_dialog_field_in_view() {
    let mut h = demo(PHONE, json!({"view": "detail"}));
    open(&mut h, "dialog.rename");
    focus(&mut h, "field:renameTemplate");
    keyboard_up(&mut h);
    let content = h.view.ctx.content_rect();
    let dialog = dialog_rect(&h).expect("the Rename dialog");
    assert!(dialog.bottom() <= content.bottom() && dialog.top() >= 0.0, "the dialog sits above the keyboard: {dialog:?}");
    let field = widget(&h, "field:renameTemplate").expect("the template field");
    assert!(field.bottom() <= content.bottom(), "the field is in view: {field:?}");
}

// ------------------------------------------------------------------ loupe gestures

fn finger(id: u64, phase: egui::TouchPhase, pos: egui::Pos2) -> egui::Event {
    egui::Event::Touch { device_id: egui::TouchDeviceId(1), id: egui::TouchId(id), phase, pos, force: None }
}

/// Two fingers about `centre`, from `from` to `to` points apart (horizontally), in `steps` moves.
fn pinch(h: &mut Headless, centre: egui::Pos2, from: f32, to: f32, steps: usize) {
    let at = |d: f32| (centre - egui::vec2(d / 2.0, 0.0), centre + egui::vec2(d / 2.0, 0.0));
    let (a, b) = at(from);
    let button = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    h.input([finger(1, egui::TouchPhase::Start, a), egui::Event::PointerMoved(a), button(a, true), finger(2, egui::TouchPhase::Start, b)]);
    h.step();
    for i in 1..=steps {
        let (a, b) = at(from + (to - from) * i as f32 / steps as f32);
        h.input([finger(1, egui::TouchPhase::Move, a), egui::Event::PointerMoved(a), finger(2, egui::TouchPhase::Move, b)]);
        h.step();
    }
    let (a, b) = at(to);
    h.input([finger(2, egui::TouchPhase::End, b), finger(1, egui::TouchPhase::End, a), button(a, false), egui::Event::PointerGone]);
    h.step();
    h.step();
}

fn loupe(h: &Headless) -> egui::Rect {
    widget(h, "canvas:image").expect("the loupe image")
}

#[test]
fn pinch_zooms_the_loupe_about_the_fingers_and_back_to_fit() {
    let mut h = demo(PHONE, json!({"view": "detail", "right": "none"}));
    let fit = loupe(&h);
    // spread two fingers over a point left of the centre: the image point there stays put
    let centre = fit.center() - egui::vec2(fit.width() / 4.0, 0.0);
    let u = (centre.x - fit.left()) / fit.width();
    pinch(&mut h, centre, 60.0, 180.0, 12);
    assert!(matches!(h.app.ui.zoom, crate::state::Zoom::Percent(_)), "zoomed: {:?}", h.app.ui.zoom);
    let zoomed = loupe(&h);
    assert!(zoomed.width() > fit.width() * 2.5, "about 3× wider: {fit:?} → {zoomed:?}");
    let u_after = (centre.x - zoomed.left()) / zoomed.width();
    assert!((u_after - u).abs() < 0.03, "the point under the fingers stayed: {u} → {u_after}");
    // pinching back in returns to Fit
    pinch(&mut h, centre, 200.0, 40.0, 12);
    assert_eq!(h.app.ui.zoom, crate::state::Zoom::Fit);
    assert_eq!(loupe(&h), fit);
}

#[test]
fn phone_tap_hides_the_editor_and_double_tap_zooms_to_100_and_back() {
    let mut h = demo(PHONE, json!({"view": "detail", "right": "edit"}));
    touch_screen(&mut h);
    let tap = |h: &mut Headless| {
        let c = loupe(h).center();
        touch_drag(h, c, c, 0);
    };
    let wait = |h: &mut Headless| {
        for _ in 0..40 {
            h.step();
        }
    };
    let with_chrome = loupe(&h);
    // one tap: the bars and the sheet go (once a second tap can no longer come), the photo takes
    // the screen, centred
    tap(&mut h);
    wait(&mut h);
    assert!(widget(&h, "icon:more").is_none() && widget(&h, "panel:sheet").is_none(), "the editor's chrome hid");
    assert_eq!(h.app.ui.right, RightPanel::Edit, "the sheet stays chosen");
    assert_eq!(h.app.ui.zoom, crate::state::Zoom::Fit, "a tap doesn't zoom");
    let alone = loupe(&h);
    assert!(alone.width() >= with_chrome.width() && alone.height() > with_chrome.height(), "larger: {with_chrome:?} → {alone:?}");
    assert!((alone.center().y - PHONE[1] / 2.0).abs() < 2.0, "centred on the screen: {alone:?}");
    tap(&mut h);
    wait(&mut h);
    assert!(widget(&h, "icon:more").is_some() && widget(&h, "panel:sheet").is_some(), "and came back");
    // two taps: 100 %, then two more: Fit; the chrome stays as it was
    tap(&mut h);
    tap(&mut h);
    wait(&mut h);
    assert_eq!(h.app.ui.zoom, crate::state::Zoom::Percent(100.0));
    assert!(widget(&h, "icon:more").is_some(), "a double tap leaves the chrome");
    tap(&mut h);
    tap(&mut h);
    wait(&mut h);
    assert_eq!(h.app.ui.zoom, crate::state::Zoom::Fit);
}

#[test]
fn phone_tap_in_crop_hides_the_chrome_but_leaves_the_photo_in_place() {
    let mut h = demo(PHONE, json!({"view": "detail", "right": "crop"}));
    touch_screen(&mut h);
    let before = loupe(&h);
    let c = before.center();
    touch_drag(&mut h, c, c, 0);
    for _ in 0..40 {
        h.step();
    }
    assert!(crate::panels::compact::chrome_hidden(&h.view.ctx), "the bars are covered");
    assert_eq!(loupe(&h), before, "the photo stays where it was");
    // a tap on the cover brings them back
    let top = egui::pos2(PHONE[0] / 2.0, 20.0);
    touch_drag(&mut h, top, top, 0);
    for _ in 0..4 {
        h.step();
    }
    assert!(!crate::panels::compact::chrome_hidden(&h.view.ctx), "back");
}

#[test]
fn trackpad_pinch_over_the_loupe_zooms() {
    let mut h = demo([1400.0, 900.0], json!({"view": "detail", "right": "none"}));
    let fit = loupe(&h);
    h.input([egui::Event::PointerMoved(fit.center()), egui::Event::Zoom(1.5)]);
    h.step();
    h.step();
    assert!(loupe(&h).width() > fit.width() * 1.4, "{fit:?} → {:?}", loupe(&h));
}

fn swipe_by(h: &mut Headless, delta: egui::Vec2) {
    let start = loupe(h).center();
    touch_drag(h, start, start + delta, 10);
}

#[test]
fn touch_swipe_at_fit_steps_through_the_photos() {
    let mut h = demo(PHONE, json!({"view": "detail", "right": "none"}));
    let first = h.app.session.active();
    swipe_by(&mut h, egui::vec2(-160.0, 6.0));
    let second = h.app.session.active();
    assert_ne!(second, first, "a swipe left shows the next photo");
    swipe_by(&mut h, egui::vec2(160.0, -4.0));
    assert_eq!(h.app.session.active(), first, "a swipe right goes back");
    // too short, or mostly vertical: no change
    swipe_by(&mut h, egui::vec2(-30.0, 0.0));
    swipe_by(&mut h, egui::vec2(-90.0, 160.0));
    assert_eq!(h.app.session.active(), first);
    // the photo is back in place once the finger lifts
    assert_eq!(loupe(&h).center().x, h.app.canvas_rect.expect("canvas").center().x);
}

#[test]
fn mouse_drag_at_fit_doesnt_change_the_photo() {
    let mut h = demo(PHONE, json!({"view": "detail", "right": "none"}));
    let first = h.app.session.active();
    let c = loupe(&h).center();
    let r = h.request("ui.drag", json!({"x": c.x, "y": c.y, "toX": c.x - 160.0, "toY": c.y, "steps": 10}), T);
    assert_eq!(r["ok"], true, "{r}");
    for _ in 0..4 {
        h.step();
    }
    assert_eq!(h.app.session.active(), first);
}

#[test]
fn a_short_dialog_keeps_its_own_height_on_the_desktop() {
    let mut h = demo([1400.0, 900.0], json!({"view": "detail"}));
    open(&mut h, "dialog.labelNames");
    let r = dialog_rect(&h).expect("label names");
    assert!(r.height() < 600.0, "sized to its contents, not the screen: {r:?}");
}

/// A phone held sideways (about 400 pt tall, from the safe area).
const PHONE_LANDSCAPE: [f32; 2] = [874.0, 402.0];

#[test]
fn phone_landscape_puts_the_editor_beside_the_photo() {
    let mut h = demo(PHONE_LANDSCAPE, json!({"view": "detail", "right": "edit"}));
    assert!(crate::panels::compact::is_compact(&h.view.ctx), "a short window is compact too");
    let sheet = widget(&h, "panel:sheet").expect("the edit sheet is up");
    assert!(sheet.width() < PHONE_LANDSCAPE[0] / 2.0 && sheet.right() >= PHONE_LANDSCAPE[0] - 1.0, "a column at the right: {sheet:?}");
    let canvas = h.app.canvas_rect.expect("the loupe is drawn");
    assert!(canvas.right() <= sheet.left() + 1.0, "the photo sits left of the sheet: {canvas:?}");
    assert!(canvas.height() > 250.0 && canvas.width() > 400.0, "the photo keeps its room: {canvas:?}");
    assert!(widget(&h, "slider:light.exposure").is_some(), "the Light sliders are in the sheet");
    // without the sheet there is no filmstrip (a swipe on the photo changes photo): the photo
    // takes the height
    click(&mut h, "icon:sheetClose");
    h.step();
    let canvas = h.app.canvas_rect.expect("the loupe");
    assert!(!h.app.widgets.iter().any(|(w, _)| w.starts_with("film:")), "no filmstrip on a phone");
    assert!(canvas.height() > 0.6 * PHONE_LANDSCAPE[1], "{canvas:?}");
    // My Photos opens as a column beside the photos, not over them
    let mut h = demo(PHONE_LANDSCAPE, json!({"view": "photoGrid"}));
    click(&mut h, "icon:sidebar");
    let left = widget(&h, "panel:left_panel").expect("My Photos is open");
    assert!(left.width() < PHONE_LANDSCAPE[0] / 2.0, "a column: {left:?}");
    // the grid shows more than one row of photos
    let cells: Vec<egui::Rect> = h.app.widgets.iter().filter(|(w, _)| w.starts_with("thumb:")).map(|(_, r)| *r).collect();
    let tops: std::collections::BTreeSet<i32> = cells.iter().map(|r| r.top() as i32).collect();
    assert!(tops.len() >= 2, "two rows of photos at least: {cells:?}");
}

#[test]
fn phone_landscape_dialogs_keep_a_readable_width() {
    let mut h = demo(PHONE_LANDSCAPE, json!({"view": "photoGrid"}));
    open(&mut h, "dialog.export");
    let r = dialog_rect(&h).expect("Export");
    assert!(r.width() <= 570.0 && r.top() >= 0.0 && r.bottom() <= PHONE_LANDSCAPE[1], "{r:?}");
}

#[test]
fn narrow_window_filter_bar_wraps_every_control_onto_the_screen() {
    // narrower than the bar's one- and two-row layouts, wider than a phone (which has the Filter sheet)
    const NARROW: [f32; 2] = [700.0, 874.0];
    let mut h = demo(NARROW, json!({"view": "photoGrid", "filterBar": true}));
    for _ in 0..4 {
        h.step();
    }
    let controls: Vec<(String, egui::Rect)> = h
        .app
        .widgets
        .iter()
        .filter(|(w, _)| w.starts_with("filter:") || w.starts_with("dropdown:filter-") || w.starts_with("button:filter"))
        .cloned()
        .collect();
    // stars, flags, labels, kind, edited, camera, lens, keyword, save, clear, presets
    assert!(controls.len() >= 20, "{controls:?}");
    for (id, r) in &controls {
        assert!(r.left() >= 0.0 && r.right() <= NARROW[0], "{id} is on screen: {r:?}");
    }
    let bar = widget(&h, "filterbar").expect("the bar");
    assert!(controls.iter().all(|(_, r)| bar.contains_rect(*r)), "inside the bar {bar:?}: {controls:?}");
}

#[test]
fn phone_touch_screen_gets_finger_sized_controls() {
    let mut h = demo(PHONE, json!({"view": "photoGrid"}));
    let mouse = h.view.ctx.global_style().spacing.interact_size.y;
    let p = egui::pos2(200.0, 400.0);
    let touch = |phase| egui::Event::Touch { device_id: egui::TouchDeviceId(1), id: egui::TouchId(1), phase, pos: p, force: None };
    h.input([touch(egui::TouchPhase::Start), touch(egui::TouchPhase::End)]);
    for _ in 0..3 {
        h.step();
    }
    let finger = h.view.ctx.global_style().spacing.interact_size.y;
    assert!(finger >= mouse + 6.0, "{mouse} → {finger}");
    // a desktop-sized window keeps the mouse sizes, touch screen or not
    let mut h = demo([1400.0, 900.0], json!({"view": "photoGrid"}));
    h.input([touch(egui::TouchPhase::Start), touch(egui::TouchPhase::End)]);
    for _ in 0..3 {
        h.step();
    }
    assert_eq!(h.view.ctx.global_style().spacing.interact_size.y, mouse);
}

#[test]
fn phone_more_menu_offers_what_the_screen_needs() {
    // the editor: the photo's settings, and Reset Edits works from there
    let mut h = demo(PHONE, json!({"view": "detail"}));
    let r = h.request("engine.execute", json!({"command": "develop.set", "params": {"control": "light.exposure", "value": 1.0}}), T);
    assert_eq!(r["ok"], true, "{r}");
    h.step();
    assert!((exposure(&h) - 1.0).abs() < 1e-9);
    click(&mut h, "icon:more");
    for id in ["more:develop.copy", "more:dialog.pasteSettings", "more:develop.pastePrevious", "more:photo.delete", "submenu:Before/After"] {
        assert!(widget(&h, id).is_some(), "{id} in the editor's menu");
    }
    for id in ["moreHeader:Edits", "moreHeader:Photo", "moreHeader:View"] {
        assert!(widget(&h, id).is_some(), "{id}: the sections' titles");
    }
    assert!(widget(&h, "more:file.addFolder").is_none(), "no library items in the editor");
    assert!(widget(&h, "more:allMenus").is_none() && widget(&h, "submenu:All Menus").is_none(), "no All Menus");
    assert!(widget(&h, "menu:all").is_none() && widget(&h, "menu:File").is_none(), "no desktop menu bar");
    click(&mut h, "more:develop.reset");
    assert!(exposure(&h).abs() < 1e-9, "Reset Edits reset the exposure");
    // the library: importing and the app
    let mut h = demo(PHONE, json!({"view": "photoGrid"}));
    click(&mut h, "icon:more");
    for id in ["more:file.addFolder", "more:library.selectAll", "more:view.people", "more:dialog.cull", "more:edit.undo", "more:app.settings"] {
        assert!(widget(&h, id).is_some(), "{id} in the library's menu");
    }
    for id in ["moreHeader:Select", "moreHeader:Add photos", "moreHeader:View options", "moreHeader:Review"] {
        assert!(widget(&h, id).is_some(), "{id}: the sections' titles");
    }
    assert!(widget(&h, "more:develop.copy").is_none(), "no editor items in the library");
    assert!(widget(&h, "more:library.emptyRecentlyDeleted").is_none(), "only in Recently Deleted");
    assert!(widget(&h, "more:allMenus").is_none() && widget(&h, "submenu:All Menus").is_none(), "no All Menus");
}

/// Every id in `items`, submenus and sections included, and the submenus' labels.
fn native_ids(items: &[crate::native_menu::NativeItem], out: &mut Vec<String>) {
    use crate::native_menu::NativeItem as N;
    for i in items {
        match i {
            N::Action { id, .. } => out.push(id.clone()),
            N::Section { items, .. } => native_ids(items, out),
            N::Submenu { label, items, .. } => {
                out.push(format!("submenu:{label}"));
                native_ids(items, out);
            }
        }
    }
}

#[test]
fn phone_more_items_hold_each_screens_commands() {
    let h = demo(PHONE, json!({"view": "photoGrid"}));
    let mut ids = Vec::new();
    native_ids(&crate::native_menu::more_items(&h.app, false), &mut ids);
    for id in [
        "view.selectMode",
        "library.selectAll",
        "file.addPhotos",
        "file.addFolder",
        "view.photoGrid",
        "view.squareGrid",
        "submenu:Grid Info",
        "view.gridInfo",
        "view.people",
        "view.slideshow",
        "dialog.cull",
        "edit.undo",
        "edit.redo",
        "app.settings",
        "app.about",
    ] {
        assert!(ids.iter().any(|i| i == id), "{id} in the library's …: {ids:?}");
    }
    assert!(!ids.iter().any(|i| i.contains("All Menus") || i == "library.emptyRecentlyDeleted"), "{ids:?}");
    let h = demo(PHONE, json!({"view": "detail"}));
    let mut ids = Vec::new();
    native_ids(&crate::native_menu::more_items(&h.app, true), &mut ids);
    for id in [
        "develop.copy",
        "dialog.copySettings",
        "develop.paste",
        "dialog.pasteSettings",
        "develop.pastePrevious",
        "develop.reset",
        "photo.rotateLeft",
        "photo.rotateRight",
        "photo.virtualCopy",
        "submenu:Add to Album",
        "dialog.newAlbum",
        "library.findSimilar",
        "dialog.rename",
        "view.histogram",
        "view.infoOverlay",
        "submenu:Before/After",
        "view.beforeAfter",
        "view.beforeAfterSplit",
        "view.beforeAfterTopBottom",
        "view.beforeAfterSplitTopBottom",
        "view.focusPoint",
        "view.faceBoxes",
        "photo.delete",
    ] {
        assert!(ids.iter().any(|i| i == id), "{id} in the editor's …: {ids:?}");
    }
    assert!(!ids.iter().any(|i| i.contains("All Menus")), "{ids:?}");
    // Recently Deleted: Empty Recently Deleted, red
    let mut h = demo(PHONE, json!({"view": "photoGrid"}));
    assert!(h.app.run("library.source", json!({"kind": "recentlyDeleted"})).is_ok());
    h.step();
    let items = crate::native_menu::more_items(&h.app, false);
    let empty = items.iter().find_map(|s| match s {
        crate::native_menu::NativeItem::Section { items, .. } => items.iter().find_map(|i| match i {
            crate::native_menu::NativeItem::Action { id, destructive, .. } if id == "library.emptyRecentlyDeleted" => Some(*destructive),
            _ => None,
        }),
        _ => None,
    });
    assert_eq!(empty, Some(true), "Empty Recently Deleted, destructive");
}

/// A finger along `points`: down at the first, through the others a frame each, up at the last.
fn touch_path(h: &mut Headless, points: &[egui::Pos2], hold_frames: usize) {
    let button = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    let (Some(&first), Some(&last)) = (points.first(), points.last()) else { return };
    h.input([finger(1, egui::TouchPhase::Start, first), egui::Event::PointerMoved(first), button(first, true)]);
    h.step();
    for _ in 0..hold_frames {
        h.step();
    }
    for &p in &points[1..] {
        h.input([finger(1, egui::TouchPhase::Move, p), egui::Event::PointerMoved(p)]);
        h.step();
    }
    h.input([finger(1, egui::TouchPhase::End, last), button(last, false), egui::Event::PointerGone]);
    h.step();
    h.step();
}

/// The grid's photos in order with the centres of those on screen.
fn grid_cells(h: &mut Headless) -> Vec<(lightcraft_catalog::PhotoId, egui::Pos2)> {
    let order = h.app.session.visible_shared().1;
    order.iter().filter_map(|id| widget(h, &format!("thumb:{}", id.0)).map(|r| (*id, r.center()))).collect()
}

/// The grid's photos whose centres are well inside the grid on screen (the grid also draws a
/// screen's worth above and below it, ready to scroll in).
fn on_screen(h: &mut Headless) -> Vec<(lightcraft_catalog::PhotoId, egui::Pos2)> {
    let view = widget(h, "grid:scroll").expect("the grid").shrink(60.0);
    grid_cells(h).into_iter().filter(|(_, p)| view.contains(*p)).collect()
}

fn selected(h: &Headless) -> Vec<lightcraft_catalog::PhotoId> {
    let mut v = h.app.session.selection.ids.clone();
    v.sort_by_key(|id| id.0);
    v
}

#[test]
fn phone_grid_tap_opens_the_photo() {
    let mut h = demo(PHONE, json!({"view": "squareGrid"}));
    touch_screen(&mut h);
    let cells = grid_cells(&mut h);
    let (id, at) = cells[4];
    touch_path(&mut h, &[at], 0);
    assert_eq!(h.app.ui.view, ViewMode::Detail, "one tap opens the photo");
    assert_eq!(h.app.session.selection.active, Some(id));
    // the same photo again, once back in the grid: still one tap
    let r = h.request("ui.set", json!({"view": "squareGrid"}), T);
    assert_eq!(r["ok"], true, "{r}");
    h.settle(SETTLE);
    let at = grid_cells(&mut h).into_iter().find(|(i, _)| *i == id).map(|(_, p)| p).expect("on screen");
    touch_path(&mut h, &[at], 0);
    assert_eq!(h.app.ui.view, ViewMode::Detail, "a selected photo opens with one tap too");
}

#[test]
fn phone_grid_long_press_starts_select_mode_without_a_menu() {
    let mut h = demo(PHONE, json!({"view": "squareGrid"}));
    touch_screen(&mut h);
    let (id, at) = grid_cells(&mut h)[2];
    touch_path(&mut h, &[at], 60);
    assert!(h.app.ui.select_mode, "a long press starts Select mode");
    assert_eq!(selected(&h), vec![id], "with that photo selected (its release doesn't take it out again)");
    assert_eq!(h.app.ui.view, ViewMode::SquareGrid, "and stays in the grid");
    assert!(widget(&h, "button:contextDeletePhoto").is_none(), "no desktop context menu");
    assert!(widget(&h, "button:selectDone").is_some() && widget(&h, "icon:selectDelete").is_some(), "the selection's bar");
    // taps now add photos and take them out
    let (other, at2) = grid_cells(&mut h)[5];
    touch_path(&mut h, &[at2], 0);
    let mut both = vec![id, other];
    both.sort_by_key(|i| i.0);
    assert_eq!(selected(&h), both);
    touch_path(&mut h, &[at2], 0);
    assert_eq!(selected(&h), vec![id]);
    click(&mut h, "button:selectDone");
    assert!(!h.app.ui.select_mode);
}

#[test]
fn phone_grid_slide_selects_a_run_of_photos() {
    let mut h = demo(PHONE, json!({"view": "squareGrid"}));
    touch_screen(&mut h);
    h.app.ui.select_mode = true;
    let r = h.request("engine.execute", json!({"command": "library.select", "params": {"ids": [], "mode": "replace"}}), T);
    assert_eq!(r["ok"], true, "{r}");
    h.step();
    let cells = grid_cells(&mut h);
    let (from, to) = (cells[1], cells[7]);
    // sideways first (that's what makes it select), then on to a photo two rows down
    let start = from.1;
    let side = start + egui::vec2(30.0, 0.0);
    let mut path = vec![start, start + egui::vec2(12.0, 0.0), side];
    for i in 1..=8 {
        path.push(side + (to.1 - side) * (i as f32 / 8.0));
    }
    touch_path(&mut h, &path, 0);
    let mut run: Vec<_> = cells[1..=7].iter().map(|(id, _)| *id).collect();
    run.sort_by_key(|i| i.0);
    assert_eq!(selected(&h), run, "every photo from the first to the last, in grid order");
    // a slide starting on a selected photo takes the run out again
    let path: Vec<_> = path.iter().rev().copied().collect();
    let back = grid_cells(&mut h);
    assert_eq!(back[7].1, to.1, "the grid didn't scroll");
    touch_path(&mut h, &[to.1, to.1 - egui::vec2(12.0, 0.0), to.1 - egui::vec2(30.0, 0.0), path[path.len() - 1]], 0);
    assert!(selected(&h).is_empty(), "{:?}", selected(&h));
}

#[test]
fn phone_grid_drag_up_scrolls_without_selecting() {
    let mut h = demo(PHONE, json!({"view": "squareGrid"}));
    touch_screen(&mut h);
    h.app.ui.select_mode = true;
    let r = h.request("engine.execute", json!({"command": "library.select", "params": {"ids": [], "mode": "replace"}}), T);
    assert_eq!(r["ok"], true, "{r}");
    h.step();
    let cells = grid_cells(&mut h);
    let start = cells[6].1;
    let path: Vec<_> = (0..=10).map(|i| start - egui::vec2(0.0, 30.0 * i as f32)).collect();
    touch_path(&mut h, &path, 0);
    assert!(selected(&h).is_empty(), "scrolling selects nothing: {:?}", selected(&h));
    let moved = grid_cells(&mut h).into_iter().find(|(id, _)| *id == cells[6].0).map(|(_, p)| p);
    assert!(moved.is_none_or(|p| p.y < start.y - 50.0), "the grid scrolled: {moved:?} from {start:?}");
}

#[test]
fn phone_grid_tap_after_a_long_press_on_the_same_photo_counts() {
    let mut h = demo(PHONE, json!({"view": "squareGrid"}));
    touch_screen(&mut h);
    let (id, at) = grid_cells(&mut h)[2];
    touch_path(&mut h, &[at], 60);
    assert_eq!(selected(&h), vec![id]);
    // the next tap on the same photo takes it out again: the long press left nothing behind
    touch_path(&mut h, &[at], 0);
    assert!(selected(&h).is_empty(), "{:?}", selected(&h));
    // and after Done, the first tap on it opens it
    touch_path(&mut h, &[at], 60);
    click(&mut h, "button:selectDone");
    touch_path(&mut h, &[at], 0);
    assert_eq!(h.app.ui.view, ViewMode::Detail, "the first tap opens the photo");
    assert_eq!(h.app.session.active(), Some(id));
}

/// A slide after a long press (finger still down) without and with Select mode already on.
fn long_press_then_slide(h: &mut Headless, from: usize, to: usize) {
    let cells = grid_cells(h);
    let (start, end) = (cells[from].1, cells[to].1);
    let side = start + egui::vec2(30.0, 0.0);
    let mut path = vec![start, start + egui::vec2(12.0, 0.0), side];
    for i in 1..=6 {
        path.push(side + (end - side) * (i as f32 / 6.0));
    }
    touch_path(h, &path, 60);
}

#[test]
fn phone_grid_knocks_for_the_long_press_and_each_photo_a_slide_selects() {
    let mut h = demo(PHONE, json!({"view": "squareGrid"}));
    let knocks = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let log = knocks.clone();
    h.app.services.haptic = Some(Box::new(move |k| log.borrow_mut().push(k)));
    touch_screen(&mut h);
    long_press_then_slide(&mut h, 1, 3);
    let k = knocks.borrow().clone();
    assert_eq!(k.first(), Some(&crate::Haptic::Press), "the long press knocks first: {k:?}");
    let ticks = k.iter().filter(|k| **k == crate::Haptic::Tick).count();
    // (a tick each time the selection changes under the finger: one step that crosses two photos
    // is one change)
    assert!((1..=2).contains(&ticks), "ticks as the slide added photos 2 and 3: {k:?}");
}

#[test]
fn select_bar_fits_a_narrow_phone_without_overlaps() {
    for width in [320.0, 360.0, 402.0] {
        let mut h = demo([width, 780.0], json!({"view": "squareGrid"}));
        assert!(h.app.run("view.selectMode", json!({"on": true})).is_ok());
        assert!(h.app.run("library.selectAll", json!({})).is_ok());
        for _ in 0..4 {
            h.step();
        }
        let none = widget(&h, "button:selectNone").or_else(|| widget(&h, "icon:selectNone")).expect("Deselect All");
        for id in ["icon:selectAlbum", "icon:selectRate", "icon:selectDelete", "button:selectDone"] {
            if let Some(r) = widget(&h, id) {
                assert!(!r.intersects(none.shrink(1.0)), "{width} pt: Deselect All over {id}: {none:?} {r:?}");
            }
        }
        assert!(none.right() <= width, "{width} pt: on screen: {none:?}");
    }
}

#[test]
fn phone_grid_long_press_then_slide_selects_the_run() {
    let mut h = demo(PHONE, json!({"view": "squareGrid"}));
    touch_screen(&mut h);
    let cells = grid_cells(&mut h);
    long_press_then_slide(&mut h, 1, 3);
    assert!(h.app.ui.select_mode);
    let mut run: Vec<_> = cells[1..=3].iter().map(|(id, _)| *id).collect();
    run.sort_by_key(|i| i.0);
    assert_eq!(selected(&h), run, "the pressed photo stays chosen and the slide adds to it");
    // in Select mode already, a long press on an unchosen photo and a slide add as well
    long_press_then_slide(&mut h, 5, 6);
    let mut more: Vec<_> = [1, 2, 3, 5, 6].iter().map(|&i| cells[i].0).collect();
    more.sort_by_key(|i| i.0);
    assert_eq!(selected(&h), more);
}

#[test]
fn phone_grid_slide_ignores_presses_on_a_dialog_above_the_grid() {
    let mut h = demo(PHONE, json!({"view": "squareGrid"}));
    touch_screen(&mut h);
    h.app.ui.select_mode = true;
    let cells = grid_cells(&mut h);
    let ids: Vec<u64> = cells[..2].iter().map(|(id, _)| id.0).collect();
    let r = h.request("engine.execute", json!({"command": "library.select", "params": {"ids": ids, "mode": "replace"}}), T);
    assert_eq!(r["ok"], true, "{r}");
    let before = selected(&h);
    let _ = h.app.run("dialog.export", json!({}));
    h.settle(SETTLE);
    let grid_layer = egui::LayerId::background();
    let covered = cells.iter().map(|(_, p)| *p).find(|p| h.view.ctx.layer_id_at(*p).is_some_and(|l| l.order != grid_layer.order));
    let Some(start) = covered else { panic!("the dialog covers some photo") };
    touch_path(&mut h, &[start, start + egui::vec2(12.0, 0.0), start + egui::vec2(40.0, 0.0), start + egui::vec2(80.0, 0.0)], 0);
    assert_eq!(selected(&h), before, "a drag on the dialog leaves the selection alone");
}

#[test]
fn phone_grid_slide_keeps_the_grid_where_it_is() {
    // a short phone (landscape-ish height), so the demo's photos are more than a screen
    let mut h = demo([402.0, 520.0], json!({"view": "squareGrid"}));
    touch_screen(&mut h);
    h.app.ui.select_mode = true;
    let cells = grid_cells(&mut h);
    // two photos chosen at the top, the second one active
    let ids: Vec<u64> = cells[..2].iter().map(|(id, _)| id.0).collect();
    let r = h.request("engine.execute", json!({"command": "library.select", "params": {"ids": ids, "active": ids[1], "mode": "replace"}}), T);
    assert_eq!(r["ok"], true, "{r}");
    h.step();
    // scroll down by finger until the top row is off screen
    let visible = on_screen(&mut h);
    let start = visible[visible.len() - 1].1;
    let path: Vec<_> = (0..=12).map(|i| start - egui::vec2(0.0, 25.0 * i as f32)).collect();
    touch_path(&mut h, &path, 0);
    h.settle(SETTLE);
    // let the fling's momentum run out before measuring where the photos are
    let mut last = h.app.grid_scroll;
    for _ in 0..600 {
        h.step();
        if h.app.grid_scroll == last {
            break;
        }
        last = h.app.grid_scroll;
    }
    let view = widget(&h, "grid:scroll").expect("the grid");
    let top = grid_cells(&mut h).into_iter().find(|(id, _)| id.0 == ids[0]).map(|(_, p)| p);
    assert!(top.is_none_or(|p| p.y < view.top()), "the chosen photos scrolled out of view: {top:?} above {view:?}");
    let shown = on_screen(&mut h);
    // slide across the first row on screen
    let row: Vec<_> = shown.iter().filter(|(_, p)| (p.y - shown[0].1.y).abs() < 1.0).copied().collect();
    assert!(row.len() >= 2, "a row of photos");
    let (a, b) = (row[0].1, row[row.len() - 1].1);
    let mut slide = vec![a, a + egui::vec2(12.0, 0.0), a + egui::vec2(30.0, 0.0)];
    slide.extend((1..=4).map(|i| a + (b - a) * (i as f32 / 4.0)));
    touch_path(&mut h, &slide, 0);
    h.settle(SETTLE);
    let after = grid_cells(&mut h);
    let pos = |id| after.iter().find(|(i, _)| *i == id).map(|(_, p)| *p);
    assert_eq!(pos(row[0].0), Some(a), "the grid didn't jump");
    assert_eq!(h.app.session.selection.active.map(|a| a.0), Some(ids[1]), "the active photo stays");
    assert_eq!(selected(&h).len(), 2 + row.len());
}

#[test]
fn phone_full_layout_touch_keeps_the_desktop_grid() {
    // a tablet or touch laptop in the full layout: a long press opens the photo's menu, a tap selects
    let mut h = demo([1400.0, 900.0], json!({"view": "squareGrid"}));
    touch_screen(&mut h);
    let (id, at) = on_screen(&mut h)[0];
    touch_path(&mut h, &[at], 60);
    assert!(!h.app.ui.select_mode, "no Select mode");
    assert!(widget(&h, "button:contextDeletePhoto").is_some(), "the photo's menu");
    h.input([egui::Event::Key { key: egui::Key::Escape, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() }]);
    h.step();
    h.step();
    let (other, at2) = on_screen(&mut h)[1];
    touch_path(&mut h, &[at2], 0);
    assert_eq!(h.app.ui.view, ViewMode::SquareGrid, "a tap doesn't open the photo");
    assert_eq!(selected(&h), vec![other], "it selects it");
    assert_ne!(id, other);
}

/// A finger held on `at` for `frames` frames; `during` runs while it is still down.
fn hold(h: &mut Headless, at: egui::Pos2, frames: usize, during: impl FnOnce(&Headless)) {
    let button = |pos, pressed| egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() };
    h.input([finger(1, egui::TouchPhase::Start, at), egui::Event::PointerMoved(at), button(at, true)]);
    for _ in 0..frames {
        h.step();
    }
    during(h);
    h.input([finger(1, egui::TouchPhase::End, at), button(at, false), egui::Event::PointerGone]);
    h.step();
    h.step();
}

#[test]
fn phone_show_original_lasts_as_long_as_the_finger() {
    let mut h = demo(PHONE, json!({"view": "detail", "right": "edit"}));
    touch_screen(&mut h);
    let at = widget(&h, "icon:showOriginal").expect("Show Original").center();
    // well past egui's long press (0.8 s)
    hold(&mut h, at, 90, |h| assert_eq!(h.app.ui.before_after, crate::state::BeforeAfter::Original, "the original while held"));
    assert_eq!(h.app.ui.before_after, crate::state::BeforeAfter::Off, "the edit once let go");
    // View > Show Original (or a Before / After layout) isn't undone by the button
    h.app.ui.before_after = crate::state::BeforeAfter::SideBySide;
    for _ in 0..3 {
        h.step();
    }
    assert_eq!(h.app.ui.before_after, crate::state::BeforeAfter::SideBySide);
    let at = widget(&h, "icon:showOriginal").expect("Show Original").center();
    hold(&mut h, at, 10, |h| assert_eq!(h.app.ui.before_after, crate::state::BeforeAfter::Original));
    assert_eq!(h.app.ui.before_after, crate::state::BeforeAfter::SideBySide, "a hold ends on the view it started from");
}

#[test]
fn phone_section_sheet_has_the_eye_and_the_auto_sync_banner() {
    let mut h = demo(PHONE, json!({"view": "detail", "right": "edit"}));
    touch_screen(&mut h);
    click(&mut h, "icon:light");
    let id = h.app.session.active().expect("a photo");
    let enabled = |h: &Headless| h.app.session.catalog.photo(id).expect("photo").develop.section_enabled("light");
    assert!(widget(&h, "icon:sheetEye").is_some(), "the section's eye");
    click(&mut h, "icon:sheetEye");
    assert!(!enabled(&h), "the eye turns the section off");
    click(&mut h, "icon:sheetEye");
    assert!(enabled(&h), "and on again");
    // Auto Sync with two photos chosen: the banner says edits go to both
    let other = h.app.session.visible_shared().1.iter().copied().find(|p| *p != id).expect("another photo");
    let r = h.request("engine.execute", json!({"command": "library.select", "params": {"ids": [id.0, other.0], "active": id.0}}), T);
    assert_eq!(r["ok"], true, "{r}");
    assert_eq!(h.app.run("develop.autoSync", json!({"on": true})).expect("auto sync")["on"], true);
    h.step();
    h.step();
    assert!(widget(&h, "button:autoSyncOff").is_some(), "the Auto Sync banner in the sheet");
}

/// What a phone host got from the Share sheet: files offered to the share sheet, and files saved
/// to the photo library, one list per export.
type Handed = std::rc::Rc<std::cell::RefCell<Vec<Vec<String>>>>;

/// [`demo`] with a phone host's export services: a background writer that writes nothing, the
/// sandbox's export folder, the share sheet and Save to Photos.
fn phone_host(ui: serde_json::Value) -> (Headless, Handed, Handed) {
    let mut h = demo(PHONE, ui);
    h.app.services.write_shared = Some(std::sync::Arc::new(|_: &str, _: &[u8]| Ok(())));
    h.app.services.export_dir = Some("/sandbox/Exports".into());
    let (shared, saved) = (Handed::default(), Handed::default());
    let (s, p) = (shared.clone(), saved.clone());
    h.app.services.share = Some(Box::new(move |paths: &[String]| s.borrow_mut().push(paths.to_vec())));
    h.app.services.save_to_photos = Some(Box::new(move |paths: &[String]| p.borrow_mut().push(paths.to_vec())));
    (h, shared, saved)
}

fn finish_export(h: &mut Headless) {
    let t0 = std::time::Instant::now();
    while h.app.export.is_some() && t0.elapsed() < SETTLE {
        h.step();
    }
    assert!(h.app.export.is_none(), "the export finished");
}

#[test]
fn phone_share_sends_a_full_size_jpeg_to_the_share_sheet() {
    let (mut h, shared, saved) = phone_host(json!({"view": "detail"}));
    click(&mut h, "icon:share");
    assert!(widget(&h, "panel:shareSheet").is_some(), "Share opens the sheet");
    assert!(widget(&h, "photo:share:0").is_some() && widget(&h, "photo:share:1").is_none(), "the editor shares its photo");
    click(&mut h, "button:shareNow");
    finish_export(&mut h);
    let shared = shared.borrow();
    assert_eq!(shared.len(), 1, "{shared:?}");
    assert!(shared[0].len() == 1 && shared[0][0].starts_with("/sandbox/Exports/") && shared[0][0].ends_with(".jpg"), "{shared:?}");
    assert!(saved.borrow().is_empty(), "nothing saved to Photos");
    assert!(widget(&h, "panel:shareSheet").is_none(), "the sheet closes");
    assert_eq!(h.app.session.last_export, None, "Export with Previous keeps what it had");
}

#[test]
fn phone_save_to_photos_takes_only_the_photos_still_chosen() {
    let (mut h, shared, saved) = phone_host(json!({"view": "photoGrid"}));
    let ids: Vec<u64> = h.app.session.visible_cloned().iter().take(3).map(|p| p.0).collect();
    let r = h.request("engine.execute", json!({"command": "library.select", "params": {"ids": ids}}), T);
    assert_eq!(r["ok"], true, "{r}");
    h.step();
    click(&mut h, "icon:share");
    assert!(widget(&h, "photo:share:2").is_some(), "the three selected photos");
    click(&mut h, "photo:share:1");
    click(&mut h, "button:shareSavePhotos");
    finish_export(&mut h);
    let saved = saved.borrow();
    assert_eq!(saved.len(), 1, "{saved:?}");
    assert_eq!(saved[0].len(), 2, "the photo taken out stays out: {saved:?}");
    assert!(shared.borrow().is_empty(), "nothing offered to the share sheet");
}

#[test]
fn phone_share_sheet_select_none_leaves_nothing_to_share() {
    let (mut h, shared, _) = phone_host(json!({"view": "detail"}));
    click(&mut h, "icon:share");
    click(&mut h, "button:shareSelectAll");
    click(&mut h, "button:shareNow");
    assert!(h.app.export.is_none(), "no export without a photo");
    assert!(widget(&h, "panel:shareSheet").is_some(), "the sheet stays");
    assert!(shared.borrow().is_empty());
}

#[test]
fn phone_export_as_more_options_opens_the_export_dialog() {
    let (mut h, _, _) = phone_host(json!({"view": "detail"}));
    let active = h.app.session.active().expect("a photo");
    click(&mut h, "icon:share");
    click(&mut h, "button:shareExportAs");
    assert!(widget(&h, "combo:shareFormat").is_some(), "the Export as form");
    assert!(widget(&h, "combo:shareSize").is_some() && widget(&h, "combo:shareQuality").is_some(), "JPEG: size and quality");
    click(&mut h, "button:shareMoreOptions");
    assert!(matches!(h.app.ui.dialog, Some(crate::state::Dialog::Export { .. })), "the full Export dialog");
    assert_eq!(h.app.session.selection.ids, vec![active], "for the photos chosen");
    assert!(widget(&h, "panel:shareSheet").is_none(), "the sheet gives way to it");
}

#[test]
fn phone_save_to_photos_needs_a_host_that_can() {
    let (mut h, _, _) = phone_host(json!({"view": "detail"}));
    h.app.services.save_to_photos = None;
    click(&mut h, "icon:share");
    assert!(widget(&h, "button:shareNow").is_some());
    assert!(widget(&h, "button:shareSavePhotos").is_none(), "no Save to Photos without the host's");
}

fn grid_phone() -> Headless {
    demo(PHONE, json!({"view": "photoGrid", "right": "none"}))
}

fn type_text(h: &mut Headless, text: &str) {
    let r = h.request("ui.text", json!({"text": text}), T);
    assert_eq!(r["ok"], true, "{r}");
    h.step();
    h.step();
}

#[test]
fn phone_filter_opens_a_sheet_not_the_desktop_bar() {
    let mut h = grid_phone();
    click(&mut h, "icon:filter");
    assert!(widget(&h, "panel:filterSheet").is_some(), "the Filter sheet");
    assert!(!h.app.ui.filter_bar, "not the desktop filter bar");
    assert!(widget(&h, "filterbar").is_none());
    // a desktop session's filter bar left on stays hidden on a phone
    h.app.ui.filter_bar = true;
    h.step();
    h.step();
    assert!(widget(&h, "filterbar").is_none());
}

#[test]
fn phone_filter_sheet_filters_as_it_is_tapped() {
    let mut h = grid_phone();
    let all = h.app.session.visible().len();
    click(&mut h, "icon:filter");
    click(&mut h, "sheetFilter:rating-3");
    assert_eq!(h.app.session.filter.rating, 3);
    click(&mut h, "sheetFilter:ratingOp-exactly");
    assert_eq!(h.app.session.filter.rating_op, lightcraft_catalog::RatingOp::Exactly);
    click(&mut h, "sheetFilter:rating-3");
    assert_eq!(h.app.session.filter.rating, 0, "the lit last star again: any rating");
    click(&mut h, "sheetFilter:flag-pick");
    click(&mut h, "sheetFilter:flag-none");
    assert_eq!(h.app.session.filter.flags, vec![lightcraft_catalog::Flag::Pick, lightcraft_catalog::Flag::None]);
    click(&mut h, "sheetFilter:label-red");
    assert_eq!(h.app.session.filter.labels, vec![lightcraft_catalog::ColorLabel::Red]);
    click(&mut h, "sheetFilter:kind-raw");
    assert_eq!(h.app.session.filter.kind, Some(lightcraft_catalog::MediaKind::Raw));
    click(&mut h, "sheetFilter:kind-panorama");
    assert_eq!((h.app.session.filter.kind, h.app.session.filter.merged.as_deref()), (None, Some("panorama")));
    click(&mut h, "sheetFilter:kind-all");
    click(&mut h, "sheetFilter:edited-edited");
    assert_eq!(h.app.session.filter.edited, Some(true));
    assert!(h.app.session.visible().len() < all);
    assert_eq!(crate::panels::filter_sheet::active_count(&h.app), 3, "flags, label, edited");
    assert!(widget(&h, "indicator:filterCount").is_some(), "the Filter button's badge");
    click(&mut h, "button:filterSheetDone");
    assert!(widget(&h, "panel:filterSheet").is_none(), "Show N photos closes the sheet");
    assert_eq!(h.app.session.filter.edited, Some(true), "and keeps the filter");
}

#[test]
fn phone_filter_sheet_picks_a_camera_from_its_list() {
    let mut h = grid_phone();
    click(&mut h, "icon:filter");
    click(&mut h, "sheetFilter:camera");
    assert!(widget(&h, "sheetFilter:camera-any").is_some(), "the camera list");
    click(&mut h, "sheetFilter:camera-0");
    let camera = h.app.session.filter.camera.clone().expect("a camera");
    assert!(widget(&h, "sheetFilter:rating-1").is_some(), "back on the filters");
    click(&mut h, "sheetFilter:camera");
    click(&mut h, "sheetFilter:camera-any");
    assert_eq!(h.app.session.filter.camera, None, "Any camera clears {camera}");
}

#[test]
fn phone_filter_reset_keeps_the_search() {
    let mut h = grid_phone();
    assert!(h.app.run("library.filter", json!({"text": "sun", "rating": 2})).is_ok());
    h.step();
    click(&mut h, "icon:filter");
    click(&mut h, "button:filterSheetReset");
    assert_eq!(h.app.session.filter.rating, 0);
    assert_eq!(h.app.session.filter.text, "sun", "the search has its own ✕");
}

#[test]
fn phone_search_takes_the_bar_and_cancel_drops_it() {
    let mut h = grid_phone();
    let all = h.app.session.visible().len();
    focus(&mut h, "field:search");
    assert!(widget(&h, "button:searchCancel").is_some(), "Cancel while typing");
    assert!(widget(&h, "icon:import").is_none(), "the field takes the whole bar");
    type_text(&mut h, "sun");
    assert_eq!(h.app.session.filter.text, "sun");
    assert!(h.app.session.visible().len() < all);
    click(&mut h, "button:searchCancel");
    assert_eq!(h.app.session.filter.text, "", "Cancel drops the search");
    assert!(h.app.ui.search.is_empty());
    assert!(widget(&h, "icon:import").is_some(), "the bar's buttons are back");
    assert_eq!(h.app.session.visible().len(), all);
}

#[test]
fn phone_search_clear_button_empties_the_field() {
    let mut h = grid_phone();
    focus(&mut h, "field:search");
    assert!(widget(&h, "button:searchClear").is_none(), "no ✕ on an empty field");
    type_text(&mut h, "sun");
    click(&mut h, "button:searchClear");
    assert_eq!(h.app.session.filter.text, "");
    assert!(widget(&h, "button:searchCancel").is_some(), "still searching");
}

#[test]
fn phone_album_row_carries_filter_and_sort() {
    let mut h = grid_phone();
    let title = widget(&h, "grid:title").expect("the album row");
    for id in ["icon:filter", "icon:sort"] {
        let r = widget(&h, id).unwrap_or_else(|| panic!("{id}"));
        assert!((r.center().y - title.center().y).abs() < 4.0, "{id} on the album row: {r:?} vs {title:?}");
        assert!(r.right() <= PHONE[0], "{id} on screen");
    }
    let count = widget(&h, "grid:count").expect("the count");
    assert!(count.right() <= widget(&h, "icon:filter").expect("filter").left(), "the count before them");
    click(&mut h, "icon:filter");
    assert!(widget(&h, "panel:filterSheet").is_some());
}

#[test]
fn phone_search_text_is_centred_in_its_field_on_a_touch_screen() {
    let mut h = grid_phone();
    touch_screen(&mut h);
    for _ in 0..3 {
        h.step();
    }
    // the field registers its text box; the bar draws the rounded field around it
    let text = widget(&h, "field:search").expect("the search field");
    let bar = widget(&h, "icon:sidebar").expect("the bar's first button");
    assert!((text.center().y - bar.center().y).abs() < 1.5, "text box {text:?} centred on the bar {bar:?}");
    assert!(text.height() < 30.0, "one line, not a finger-sized box: {text:?}");
}

#[test]
fn phone_library_has_no_bottom_bar_but_select_mode_keeps_its_own() {
    let mut h = grid_phone();
    for id in ["icon:photoGrid", "icon:squareGrid", "icon:detail", "button:selectMode"] {
        assert!(widget(&h, id).is_none(), "{id}: a pinch sizes the grid, a long press selects");
    }
    let more = h.request("ui.clickWidget", json!({"id": "icon:more"}), T);
    assert_eq!(more["ok"], true, "{more}");
    h.step();
    click(&mut h, "more:view.selectMode");
    assert!(h.app.ui.select_mode, "… → Select Photos");
    assert!(widget(&h, "icon:selectDelete").is_some(), "Select mode's bar");
    assert!(widget(&h, "icon:selectShare").is_none(), "Share is the top bar's");
    assert!(widget(&h, "icon:share").is_some());
}

#[test]
fn phone_grid_months_are_heading_rows_without_half_empty_rows() {
    let h = demo(PHONE, json!({"view": "photoGrid", "right": "none", "sort": {"group": "month"}}));
    let headers: Vec<egui::Rect> = h.app.widgets.iter().filter(|(w, _)| w.starts_with("group:")).map(|(_, r)| *r).collect();
    assert!(headers.len() >= 2, "a heading per month on screen: {headers:?}");
    assert!(headers.iter().all(|r| r.width() > PHONE[0] * 0.8), "headings are full-width rows: {headers:?}");
}

#[test]
fn phone_add_border_shares_the_photo_framed() {
    let (mut h, shared, _) = phone_host(json!({"view": "detail"}));
    let written: std::sync::Arc<std::sync::Mutex<Vec<u8>>> = Default::default();
    let w = written.clone();
    h.app.services.write_shared = Some(std::sync::Arc::new(move |_: &str, bytes: &[u8]| {
        *w.lock().unwrap() = bytes.to_vec();
        Ok(())
    }));
    click(&mut h, "icon:share");
    click(&mut h, "button:shareBorder");
    assert!(widget(&h, "image:borderPreview").is_some(), "the border page");
    click(&mut h, "button:borderShape2");
    click(&mut h, "button:borderColor4");
    click(&mut h, "button:shareBorderShare");
    finish_export(&mut h);
    assert_eq!(shared.borrow().len(), 1, "to the share sheet");
    let bytes = written.lock().unwrap().clone();
    let img = lightcraft_codecs::decode(&bytes, Default::default()).expect("a JPEG").to_srgb8();
    assert_eq!(img.width, img.height, "1:1");
    let [r, g, b, _] = img.get(0, 0);
    assert!(r < 16 && g < 16 && b < 16, "black border: {:?}", img.get(0, 0));
    assert!(widget(&h, "panel:shareSheet").is_none(), "the sheet closes");
    assert_eq!(h.app.session.last_export, None, "Export with Previous keeps what it had");
}

#[test]
fn phone_more_sheet_is_rows_and_pages_a_finger_can_use() {
    let mut h = grid_phone();
    click(&mut h, "icon:more");
    assert!(widget(&h, "panel:moreSheet").is_some(), "… is a sheet");
    let row = widget(&h, "more:view.selectMode").expect("a row");
    assert!(row.height() >= 44.0 && row.width() > PHONE[0] * 0.8, "full-width finger rows: {row:?}");
    let head = widget(&h, "moreHeader:Select").expect("the section's title");
    assert!(head.bottom() <= row.top() + 0.5, "over its rows: {head:?} {row:?}");
    // a submenu is the next page, its items keep their parameters
    click(&mut h, "submenu:Grid Info");
    assert!(widget(&h, "more:view.selectMode").is_none(), "Grid Info's page replaces the first");
    click(&mut h, r#"more:view.gridInfo|{"info":"exposure"}"#);
    assert_eq!(h.app.ui.grid_info, "exposure", "an item runs its command with its parameters");
    assert!(widget(&h, "panel:moreSheet").is_none(), "and the sheet closes");
    // ‹ goes back a page, ✕ closes
    click(&mut h, "icon:more");
    click(&mut h, "submenu:Grid Info");
    click(&mut h, "icon:moreSheetClose");
    assert!(widget(&h, "more:view.selectMode").is_some(), "back on the first page");
    click(&mut h, "more:view.squareGrid");
    assert_eq!(h.app.ui.view, ViewMode::SquareGrid, "a row runs its command");
    assert!(widget(&h, "panel:moreSheet").is_none(), "and the sheet closes");
}

#[test]
fn phone_select_bar_deselects_and_restores_in_recently_deleted() {
    let mut h = grid_phone();
    assert!(h.app.run("view.selectMode", json!({"on": true})).is_ok());
    assert!(h.app.run("library.selectAll", json!({})).is_ok());
    h.step();
    h.step();
    assert!(widget(&h, "icon:selectDelete").is_some());
    let deselect = widget(&h, "button:selectNone").expect("Deselect All");
    assert!(deselect.height() >= 44.0, "a finger's height: {deselect:?}");
    assert!(widget(&h, "button:selectRestore").is_none() && widget(&h, "icon:selectDeletePermanently").is_none());
    let before = h.app.session.selection.ids.len();
    click(&mut h, "button:selectNone");
    // (as the desktop's Deselect All: the active photo stays)
    let after = h.app.session.selection.ids.len();
    assert!(before > 1 && after <= 1, "Deselect All: {before} → {after}");
    // delete two, then select them in Recently Deleted
    let ids: Vec<_> = h.app.session.visible().iter().take(2).copied().collect();
    assert_eq!(ids.len(), 2);
    let raw: Vec<u64> = ids.iter().map(|i| i.0).collect();
    assert!(h.app.run("photo.delete", json!({"ids": raw})).is_ok());
    assert!(h.app.run("library.source", json!({"kind": "recentlyDeleted"})).is_ok());
    assert!(h.app.run("view.selectMode", json!({"on": true})).is_ok());
    assert!(h.app.run("library.selectAll", json!({})).is_ok());
    h.step();
    h.step();
    assert!(widget(&h, "icon:selectDelete").is_none(), "no Delete in Recently Deleted");
    assert!(widget(&h, "icon:selectDeletePermanently").is_some(), "Delete Permanently");
    click(&mut h, "button:selectRestore");
    assert!(ids.iter().all(|&id| h.app.session.catalog.photo(id).is_some_and(|p| !p.deleted)), "Restore brings them back");
}

#[test]
fn phone_about_has_help_rows_desktop_none() {
    fn draw(app: &mut LightcraftApp, ctx: &egui::Context, compact: bool, input: egui::RawInput) -> Vec<(String, egui::Rect)> {
        ctx.data_mut(|d| d.insert_temp(egui::Id::unique("compact-layout"), compact));
        let mut out = ctx.run_ui(input, |ui| crate::panels::compact::about_help_rows(app, ui));
        out.textures_delta.clear();
        crate::widgets::take_registry(ctx)
    }
    let mut app = LightcraftApp::new(lightcraft_engine::Session::with_demo(), Services { png: None, ..Default::default() });
    let ctx = egui::Context::default();
    assert!(draw(&mut app, &ctx, false, Default::default()).is_empty(), "the desktop keeps its Help menu");
    let rows = draw(&mut app, &ctx, true, Default::default());
    for id in ["about:app.whatsNew", "about:app.feedback", "about:app.help", "about:app.systemInfo"] {
        let r = rows.iter().find(|(w, _)| w == id).map(|(_, r)| *r).unwrap_or_else(|| panic!("{id}: {rows:?}"));
        assert!(r.height() >= 44.0, "{id}: a finger's row {r:?}");
    }
    // a tap on What's New opens it
    let at = rows.iter().find(|(w, _)| w == "about:app.whatsNew").map(|(_, r)| r.center()).unwrap();
    let press = |pressed| egui::RawInput {
        events: vec![
            egui::Event::PointerMoved(at),
            egui::Event::PointerButton { pos: at, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() },
        ],
        ..Default::default()
    };
    draw(&mut app, &ctx, true, press(true));
    draw(&mut app, &ctx, true, press(false));
    assert!(matches!(app.ui.dialog, Some(crate::state::Dialog::WhatsNew)), "What's New opened");
}

#[test]
fn desktop_keeps_its_menu_bar() {
    let h = demo([1400.0, 900.0], json!({"view": "photoGrid"}));
    assert!(!crate::panels::compact::is_compact(&h.view.ctx));
    for id in ["menu:File", "menu:Edit", "menu:View", "menu:Photo", "menu:Help"] {
        assert!(widget(&h, id).is_some(), "{id}: the desktop's menu bar");
    }
    assert!(widget(&h, "panel:moreSheet").is_none() && widget(&h, "button:selectNone").is_none(), "no phone sheet or select bar");
}

#[test]
fn phone_sort_is_a_sheet_of_finger_sized_choices() {
    let mut h = grid_phone();
    click(&mut h, "icon:sort");
    assert!(widget(&h, "panel:sortSheet").is_some(), "Sort opens a sheet");
    let row = widget(&h, "sheetSort:key-fileName").expect("a row per key");
    assert!(row.height() >= 44.0, "{row:?}");
    click(&mut h, "sheetSort:key-fileName");
    assert_eq!(h.app.session.sort.key, lightcraft_catalog::SortKey::FileName);
    click(&mut h, "sheetSort:asc");
    assert!(h.app.session.sort.ascending);
    click(&mut h, "sheetSort:group-year");
    assert_eq!(h.app.session.sort.group, lightcraft_catalog::GroupBy::Year);
    click(&mut h, "sheetSort:key-random");
    assert!(widget(&h, "sheetSort:asc").is_none(), "no order for a shuffle");
    assert!(widget(&h, "sheetSort:reshuffle").is_some());
    click(&mut h, "button:sortSheetDone");
    assert!(widget(&h, "panel:sortSheet").is_none());
}

#[test]
fn phone_my_photos_drawer_has_finger_sized_rows() {
    let mut h = grid_phone();
    click(&mut h, "icon:sidebar");
    let row = widget(&h, "source:all").expect("All Photos in the drawer");
    assert!(row.height() >= 44.0, "a finger's row: {row:?}");
    // the desktop keeps its compact rows
    let d = demo([1400.0, 900.0], json!({"view": "photoGrid", "leftPanel": true}));
    let row = widget(&d, "source:all").expect("All Photos in the sidebar");
    assert!(row.height() < 32.0, "{row:?}");
}

#[test]
fn phone_long_press_menus_are_a_sheet_with_inline_submenus() {
    let mut h = grid_phone();
    click(&mut h, "icon:sidebar");
    let r = h.request("ui.clickWidget", json!({"id": "source:album:6", "button": "right"}), T);
    assert_eq!(r["ok"], true, "{r}");
    h.step();
    h.step();
    let sheet = widget(&h, "panel:contextSheet").expect("the album's menu as a sheet");
    assert!(sheet.width() > PHONE[0] - 40.0, "as wide as the screen: {sheet:?}");
    assert!(sheet.bottom() > PHONE[1] - 40.0, "along its bottom: {sheet:?}");
    let rows = widget(&h, "menu-rows:1").expect("its rows");
    // a submenu opens under its row and the sheet stays
    let before = rows.height();
    click(&mut h, "button:albumImport");
    assert!(widget(&h, "panel:contextSheet").is_some(), "a submenu row keeps the sheet open");
    let after = widget(&h, "menu-rows:1").expect("its rows").height();
    assert!(after > before, "the submenu's rows open inline: {before} → {after}");
    // a tap on an item closes it
    let first = widget(&h, "menu-rows:1").expect("its rows");
    let r = h.request("ui.click", json!({"x": first.left() + 60.0, "y": first.top() + 22.0}), T);
    assert_eq!(r["ok"], true, "{r}");
    h.step();
    h.step();
    assert!(widget(&h, "panel:contextSheet").is_none(), "an item closes the sheet");
}

#[test]
fn phone_rate_sheet_rates_flags_and_labels_by_finger() {
    // the editor's photo
    let mut h = demo(PHONE, json!({"view": "detail"}));
    let id = h.app.session.active().expect("a photo");
    let photo = |h: &Headless| h.app.session.catalog.photo(id).cloned().expect("the photo");
    click(&mut h, "icon:rate");
    assert!(widget(&h, "panel:rateSheet").is_some(), "★ opens the Rate sheet");
    click(&mut h, "rate:star-4");
    assert_eq!(photo(&h).rating, 4);
    click(&mut h, "rate:star-4");
    assert_eq!(photo(&h).rating, 0, "the lit last star again clears it");
    click(&mut h, "rate:flag-reject");
    assert_eq!(photo(&h).flag, lightcraft_catalog::Flag::Reject);
    click(&mut h, "rate:label-purple");
    assert_eq!(photo(&h).label, Some(lightcraft_catalog::ColorLabel::Purple));
    click(&mut h, "rate:label-none");
    assert_eq!(photo(&h).label, None);
    assert_eq!(h.app.session.active(), Some(id), "the editor stays on its photo");
    click(&mut h, "button:rateSheetDone");
    assert!(widget(&h, "panel:rateSheet").is_none());
    // a selection, from Select mode
    let mut h = grid_phone();
    let ids: Vec<u64> = h.app.session.visible_cloned().iter().take(3).map(|p| p.0).collect();
    let r = h.request("engine.execute", json!({"command": "library.select", "params": {"ids": ids}}), T);
    assert_eq!(r["ok"], true, "{r}");
    assert!(h.app.run("view.selectMode", json!({"on": true})).is_ok());
    h.step();
    h.step();
    click(&mut h, "icon:selectRate");
    click(&mut h, "rate:star-2");
    for id in &ids {
        assert_eq!(h.app.session.catalog.photo(lightcraft_catalog::PhotoId(*id)).expect("photo").rating, 2, "photo {id}");
    }
}

#[test]
fn phone_dialogs_have_finger_sized_controls() {
    let mut h = grid_phone();
    assert!(h.app.run("dialog.newAlbum", json!({})).is_ok());
    h.step();
    h.step();
    for id in ["field:albumName", "check:albumIncludeSelected", "button:dialogCancel", "button:dialogOk"] {
        let r = widget(&h, id).unwrap_or_else(|| panic!("{id}"));
        assert!(r.height() >= 44.0, "{id} is a finger's height: {r:?}");
    }
    // the desktop's dialog keeps its size
    let mut d = demo([1400.0, 900.0], json!({"view": "photoGrid"}));
    assert!(d.app.run("dialog.newAlbum", json!({})).is_ok());
    d.step();
    d.step();
    let ok = widget(&d, "button:dialogOk").expect("Create");
    assert!(ok.height() <= 32.0, "{ok:?}");
}

#[test]
fn phone_settings_is_a_list_of_sections_then_a_page() {
    let mut h = grid_phone();
    assert!(h.app.run("app.settings", json!({})).is_ok());
    h.step();
    h.step();
    let row = widget(&h, "button:settingsTab-display").expect("the sections as rows");
    assert!(row.height() >= 44.0, "{row:?}");
    assert!(widget(&h, "settings:page").is_none(), "no page until a section is chosen");
    click(&mut h, "button:settingsTab-general");
    assert!(widget(&h, "settings:page").is_some(), "General's page");
    assert!(widget(&h, "button:settingsTab-display").is_none(), "the list gives way to it");
    let english = h.app.widgets.iter().find(|(w, _)| w.starts_with("button:lang-") || w == "button:language-en").map(|(_, r)| *r);
    if let Some(r) = english {
        assert!(r.height() >= 44.0, "finger-sized choices: {r:?}");
    }
    click(&mut h, "button:settingsBack");
    assert!(widget(&h, "button:settingsTab-display").is_some(), "‹ back to the sections");
}

#[test]
fn phone_dialog_dropdowns_list_finger_sized_rows() {
    let mut h = demo(PHONE, json!({"view": "detail"}));
    assert!(h.app.run("dialog.export", json!({})).is_ok());
    h.step();
    h.step();
    click(&mut h, "combo:exportPreset");
    // the list's rows: the items egui laid out in the pop-up below the dropdown
    let combo = widget(&h, "combo:exportPreset").expect("the dropdown");
    // the pop-up below the dropdown holds the four built-in presets
    let list = h.view.ctx.memory(|m| {
        m.areas()
            .visible_layer_ids()
            .into_iter()
            .filter_map(|l| m.area_rect(l.id))
            .filter(|r| r.top() >= combo.bottom() - 1.0 && (r.left() - combo.left()).abs() < 2.0)
            .max_by(|a, b| a.height().total_cmp(&b.height()))
    });
    let list = list.expect("its list is open below it");
    assert!(list.height() >= 4.0 * 44.0, "finger-sized rows: {list:?}");
    assert!(combo.height() >= 44.0, "{combo:?}");
    assert!(!crate::widgets::set_finger_scope(false), "the dialog put the finger scope back");
}

#[test]
fn phone_editor_has_no_filmstrip_between_the_photo_and_its_tools() {
    let h = demo(PHONE, json!({"view": "detail"}));
    assert!(h.app.ui.filmstrip, "the desktop's filmstrip setting is on");
    assert!(!h.app.widgets.iter().any(|(w, _)| w.starts_with("film:")), "but a phone shows none: a swipe on the photo changes photo");
    // the desktop keeps it
    let d = demo([1400.0, 900.0], json!({"view": "detail"}));
    assert!(d.app.widgets.iter().any(|(w, _)| w.starts_with("film:")), "the desktop's filmstrip");
}

#[test]
fn phone_held_sideways_shows_the_photo_alone_turned() {
    let mut h = demo(PHONE, json!({"view": "detail", "right": "none"}));
    let id = h.app.session.active().expect("a photo");
    assert!(widget(&h, "photo:sideways").is_none(), "upright: the editor");
    h.app.device_turn = 1;
    h.step();
    h.step();
    let shown = widget(&h, "photo:sideways").expect("sideways: the photo alone");
    // a quarter turn swaps the photo's sides on the screen
    let p = h.app.session.catalog.photo(id).expect("photo");
    let (w, hgt) = lightcraft_pipeline::native_output_size(p.width.max(1) as usize, p.height.max(1) as usize, &p.develop);
    let photo_wide = w >= hgt;
    assert_eq!(shown.height() > shown.width(), photo_wide, "turned: {shown:?} for a {w}×{hgt} photo");
    assert!(shown.width() <= PHONE[0] + 0.5 && shown.height() <= PHONE[1] + 0.5, "it fits the screen: {shown:?}");
    // a tool's sheet open: editing stays upright
    h.app.ui.right = RightPanel::Edit;
    h.step();
    h.step();
    assert!(widget(&h, "photo:sideways").is_none(), "editing stays upright");
    // turned back: the editor
    h.app.ui.right = RightPanel::None;
    h.app.device_turn = 0;
    h.step();
    h.step();
    assert!(widget(&h, "photo:sideways").is_none());
}

#[test]
fn phone_tool_panels_have_finger_sized_controls_under_one_title() {
    let h = demo(PHONE, json!({"view": "detail", "right": "crop"}));
    // (the phone's own Crop: `phone_crop_is_chips_and_round_tools`)
    for id in ["button:cropRotateLeft", "button:cropFlipH", "button:straightenTool"] {
        let r = widget(&h, id).unwrap_or_else(|| panic!("{id}"));
        assert!(r.height() >= 44.0, "{id} is a finger's height: {r:?}");
    }
    // the desktop's Crop keeps its compact buttons
    let d = demo([1400.0, 900.0], json!({"view": "detail", "right": "crop"}));
    let r = widget(&d, "button:cropRotateLeft").expect("Rotate Left");
    assert!(r.height() < 30.0, "{r:?}");
    assert!(!crate::widgets::set_finger_scope(false), "the sheet put the finger scope back");
}

#[test]
fn phone_keyword_list_fits_the_sheet_with_finger_sized_rows() {
    let mut h = grid_phone();
    h.app.ui.right = RightPanel::Keywords;
    h.step();
    h.step();
    let sheet = widget(&h, "panel:sheet").expect("the Keywords sheet");
    for id in ["field:keywordFilter", "keywordList:create", "keywordList:delete"] {
        let r = widget(&h, id).unwrap_or_else(|| panic!("{id}"));
        assert!(r.right() <= sheet.right() + 0.5, "{id} inside the sheet: {r:?} in {sheet:?}");
    }
    assert!(widget(&h, "keywordList:create").expect("+").height() >= 44.0);
    let row = h.app.widgets.iter().find(|(w, _)| w.starts_with("keywordRow:")).map(|(_, r)| *r).expect("a keyword row");
    assert!(row.height() >= 44.0, "{row:?}");
    let count = h.app.widgets.iter().find(|(w, _)| w.starts_with("keywordCount:")).map(|(_, r)| *r).expect("its count");
    assert!(count.right() <= sheet.right(), "the count is on screen: {count:?}");
    // the desktop keeps its dense list
    let mut d = demo([1400.0, 900.0], json!({"view": "photoGrid"}));
    d.app.ui.right = RightPanel::Keywords;
    d.step();
    d.step();
    let row = d.app.widgets.iter().find(|(w, _)| w.starts_with("keywordRow:")).map(|(_, r)| *r).expect("a keyword row");
    assert!(row.height() < 30.0, "{row:?}");
}

#[test]
fn phone_presets_list_shows_looks_in_finger_sized_rows() {
    let mut h = demo(PHONE, json!({"view": "detail", "right": "none"}));
    assert!(!h.app.ui.preset_thumbs, "the desktop's thumbnails choice is off");
    click(&mut h, "icon:presets");
    let row = h.app.widgets.iter().find(|(w, _)| w.starts_with("preset:")).map(|(_, r)| *r).expect("a preset row");
    assert!(row.height() >= 60.0, "a thumbnail row a finger can take: {row:?}");
    let group = h.app.widgets.iter().find(|(w, _)| w.starts_with("presetGroup:")).map(|(_, r)| *r).expect("a group heading");
    assert!(group.height() >= 44.0, "{group:?}");
    // + in the sheet's header, beside its title and Hide (no strip of its own)
    let hide = widget(&h, "icon:sheetClose").expect("the sheet's Hide");
    let r = widget(&h, "icon:presetAdd").expect("+");
    assert!(r.height() >= 36.0 && (r.center().y - hide.center().y).abs() < 1.0, "+ in the header: {r:?}");
    assert!(r.bottom() <= group.top(), "+ above the list");
    // the desktop keeps its dense list
    let d = demo([1400.0, 900.0], json!({"view": "detail", "presets": true}));
    if let Some(row) = d.app.widgets.iter().find(|(w, _)| w.starts_with("preset:")).map(|(_, r)| *r) {
        assert!(row.height() < 30.0, "{row:?}");
    }
}

#[test]
fn phone_compare_stacks_the_photos_over_a_bar_of_buttons() {
    let mut h = demo(PHONE, json!({"view": "compare", "right": "none"}));
    let (sel, cand) = crate::panels::compare::compare_pair(&mut h.app).expect("a pair to compare");
    let (top, bottom) = (widget(&h, &format!("cull:{}", sel.0)).expect("Select"), widget(&h, &format!("cull:{}", cand.0)).expect("Candidate"));
    assert!(top.bottom() <= bottom.top(), "Select above Candidate: {top:?} {bottom:?}");
    assert!(!h.app.widgets.iter().any(|(w, _)| w.starts_with("film:")), "no filmstrip on a phone");
    for id in ["button:comparePrevious", "button:compareSwap", "button:compareRate", "button:compareMakeSelect", "button:compareNext"] {
        assert!(widget(&h, id).unwrap_or_else(|| panic!("{id}")).height() >= 44.0, "{id}");
    }
    click(&mut h, "button:compareSwap");
    assert_eq!(crate::panels::compare::compare_pair(&mut h.app), Some((cand, sel)), "Swap");
    click(&mut h, "button:compareNext");
    let (_, next) = crate::panels::compare::compare_pair(&mut h.app).expect("a pair");
    assert_ne!(next, sel, "Next brings another candidate");
    click(&mut h, "button:compareRate");
    assert!(widget(&h, "panel:rateSheet").is_some(), "Rate opens the Rate sheet");
}

#[test]
fn phone_survey_has_a_bar_for_the_photo_last_tapped() {
    let mut h = grid_phone();
    let ids: Vec<u64> = h.app.session.visible_cloned().iter().take(4).map(|p| p.0).collect();
    let r = h.request("engine.execute", json!({"command": "library.select", "params": {"ids": ids}}), T);
    assert_eq!(r["ok"], true, "{r}");
    h.app.ui.view = ViewMode::Survey;
    touch_screen(&mut h);
    for id in ["button:surveyRate", "button:surveyRemove", "button:surveyEdit"] {
        assert!(widget(&h, id).unwrap_or_else(|| panic!("{id}")).height() >= 44.0, "{id}");
    }
    let x = widget(&h, &format!("button:surveyX:{}", ids[1])).expect("each photo's ✕ on a touch screen");
    assert!(x.width() >= 44.0 && x.height() >= 44.0, "a finger's ✕: {x:?}");
    click(&mut h, "button:surveyRate");
    assert!(widget(&h, "panel:rateSheet").is_some(), "Rate opens the Rate sheet");
    click(&mut h, "button:rateSheetDone");
    let before = h.app.session.selection.ids.len();
    click(&mut h, "button:surveyRemove");
    assert_eq!(h.app.session.selection.ids.len(), before - 1, "Remove takes a photo out of the survey");
    click(&mut h, "button:surveyEdit");
    assert_eq!(h.app.ui.view, ViewMode::Detail, "Edit opens the editor");
}

#[test]
fn phone_people_setup_button_is_full_width_and_finger_sized() {
    let mut h = demo(PHONE, json!({"view": "people", "right": "none"}));
    // (shown while recognition can run but has no model yet)
    let ctx = h.view.ctx.clone();
    crate::dialogs::faces::pretend_no_model_yet(&mut h.app, &ctx);
    h.step();
    h.step();
    let b = widget(&h, "faces:setup").expect("Set up face recognition");
    assert!(b.height() >= 44.0 && b.width() > PHONE[0] * 0.8, "a full-width finger's button: {b:?}");
    let mut d = demo([1400.0, 900.0], json!({"view": "people"}));
    let ctx = d.view.ctx.clone();
    crate::dialogs::faces::pretend_no_model_yet(&mut d.app, &ctx);
    d.step();
    d.step();
    let b = widget(&d, "faces:setup").expect("Set up face recognition");
    assert!(b.height() < 32.0, "the desktop keeps its button: {b:?}");
}

/// The phone app (`Services::mobile`), held upright, Settings open on `tab`.
fn mobile_settings(tab: &str) -> Headless {
    let services = Services { png: None, mobile: true, ..Default::default() };
    let app = LightcraftApp::new(lightcraft_engine::Session::with_demo(), services);
    let mut h = Headless::new(app, PHONE, 1.0);
    let r = h.request("ui.set", json!({"view": "photoGrid", "right": "none"}), T);
    assert_eq!(r["ok"], true, "{r}");
    h.settle(SETTLE);
    assert!(h.app.run("app.settings", json!({"tab": tab})).is_ok());
    // (past the sheet's slide up: headless time moves a frame at a time)
    for _ in 0..60 {
        h.step();
    }
    h
}

fn any_widget(h: &Headless, prefix: &str) -> bool {
    h.app.widgets.iter().any(|(w, _)| w.starts_with(prefix))
}

#[test]
fn mobile_settings_leave_out_what_only_a_computer_has() {
    // the section list: no Display (the system manages a phone's colour)
    let mut h = mobile_settings("general");
    assert!(widget(&h, "button:settingsTab-general").is_some(), "the sections");
    assert!(widget(&h, "button:settingsTab-display").is_none(), "no Display section");
    click(&mut h, "button:settingsTab-general");
    // General: a language dropdown, no library folders, startup or dialog choices
    let lang = widget(&h, "combo:settingsLanguage").expect("the language dropdown");
    assert!(lang.height() >= 44.0, "{lang:?}");
    assert!(!any_widget(&h, "button:settingsLanguage-"), "not a row of every language");
    assert!(widget(&h, "button:settingsOpenLibrary").is_none(), "no library folder");
    assert!(widget(&h, "check:settings.autoAdvance").is_some(), "Culling stays");
    assert!(widget(&h, "check:settings.askBeforeMove").is_none());
    assert!(widget(&h, "check:settings.closeDialogsOutside").is_none());

    // Performance: the cache's size on a slider, no GPU or memory choices, no smart preview folder
    let mut h = mobile_settings("performance");
    assert!(widget(&h, "slider:settingsCache").is_some(), "the cache slider");
    // (the demo library isn't on disk, so it has no smart previews to count)
    let on_disk = h.app.run("library.smartPreviewsLocation", json!({})).is_ok();
    assert_eq!(widget(&h, "text:settingsSmartSize").is_some(), on_disk, "the smart previews' size where there are some");
    for gone in ["check:settings.gpu", "button:settingsPreview-", "button:settingsMemory-", "button:settingsCache-", "button:settingsSmartChoose"] {
        assert!(!any_widget(&h, gone), "{gone} is a computer's");
    }

    // Import: no XMP sidecars, dropped files or cards
    let h = mobile_settings("import");
    assert!(widget(&h, "settings:page").is_some());
    for gone in ["check:settings.autoWriteXmp", "check:settings.importDropped", "check:settings.importOnCard", "button:settingsAutoFolder"] {
        assert!(widget(&h, gone).is_none(), "{gone} is a computer's");
    }

    // a link to Display shows General
    let h = mobile_settings("display");
    assert!(widget(&h, "combo:settingsLanguage").is_some(), "General instead");
}

#[test]
fn desktop_settings_keep_every_section() {
    let mut h = demo([1400.0, 900.0], json!({"view": "photoGrid"}));
    assert!(h.app.run("app.settings", json!({"tab": "general"})).is_ok());
    h.step();
    h.step();
    assert!(widget(&h, "button:settingsTab-display").is_some());
    assert!(any_widget(&h, "button:settingsLanguage-"), "every language as a choice");
    assert!(widget(&h, "combo:settingsLanguage").is_none());
    assert!(widget(&h, "button:settingsOpenLibrary").is_some());
    assert!(h.app.run("app.settings", json!({"tab": "performance"})).is_ok());
    h.step();
    h.step();
    assert!(widget(&h, "check:settings.gpu").is_some());
    assert!(widget(&h, "slider:settingsCache").is_none());
}

/// A phone app whose host shows "…" as its own menu: what the host was asked for, each frame.
fn native_menu_phone() -> (Headless, std::sync::Arc<std::sync::Mutex<Vec<crate::native_menu::MenuRequest>>>) {
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let log = seen.clone();
    let services = Services {
        png: None,
        mobile: true,
        menu_button: Some(Box::new(move |r| log.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(r))),
        ..Default::default()
    };
    let app = LightcraftApp::new(lightcraft_engine::Session::with_demo(), services);
    let mut h = Headless::new(app, PHONE, 1.0);
    let r = h.request("ui.set", json!({"view": "photoGrid", "right": "none"}), T);
    assert_eq!(r["ok"], true, "{r}");
    h.settle(SETTLE);
    (h, seen)
}

#[test]
fn phone_more_is_the_hosts_menu_over_the_dots() {
    let (mut h, seen) = native_menu_phone();
    h.step();
    let last = seen.lock().unwrap_or_else(std::sync::PoisonError::into_inner).last().cloned();
    let more = last.and_then(|r| r.into_iter().find(|b| b.key == "more"));
    let crate::native_menu::MenuButton { rect, items, .. } = more.expect("the host is asked to show its menu");
    assert_eq!(Some(rect), widget(&h, "icon:more"), "over the drawn …");
    // the library's sections
    let has = |id: &str| {
        fn walk(items: &[crate::native_menu::NativeItem], id: &str) -> bool {
            use crate::native_menu::NativeItem as N;
            items.iter().any(|i| match i {
                N::Action { id: i, .. } => i == id,
                N::Section { items, .. } | N::Submenu { items, .. } => walk(items, id),
            })
        }
        walk(&items, id)
    };
    assert!(has("app.settings") && has("view.selectMode") && has("file.addPhotos"));
    assert!(has("view.slideshow") && has("dialog.cull") && has("edit.undo"), "the library's own commands");
    assert!(!has("app.shortcuts") && !has("file.export"), "no All Menus: {items:?}");
    // a dialog over it: the native button steps aside
    assert!(h.app.run("app.about", json!({})).is_ok());
    h.step();
    let last = seen.lock().unwrap_or_else(std::sync::PoisonError::into_inner).last().cloned();
    assert!(last.is_some_and(|r| r.is_empty()), "hidden under a dialog");
    h.app.ui.dialog = None;
    // a pick runs on the next frame
    crate::native_menu::pick("app.settings", json!({}));
    h.step();
    assert!(matches!(h.app.ui.dialog, Some(crate::state::Dialog::Settings { .. })), "{:?}", h.app.ui.dialog.is_some());
}

#[test]
fn phone_settings_is_a_sheet_from_the_bottom_edge() {
    let mut h = mobile_settings("general");
    // (past its slide up: headless time moves a frame at a time)
    for _ in 0..60 {
        h.step();
    }
    let w = widget(&h, "dialog:window").expect("the Settings window");
    assert!((w.width() - PHONE[0]).abs() < 1.0, "the width of the screen: {w:?}");
    assert!((w.bottom() - PHONE[1]).abs() < 1.0, "down to the bottom edge: {w:?}");
    assert!(w.top() < 40.0, "up to the status bar: {w:?}");
    assert!(widget(&h, "button:settingsClose").is_some(), "its own ✕");
    assert!(widget(&h, "button:dialogOk").is_none(), "no Close button at the bottom");
    click(&mut h, "button:settingsClose");
    assert!(h.app.ui.dialog.is_none(), "✕ closes it");
}

#[test]
fn phone_about_is_a_sheet_like_settings() {
    let mut h = grid_phone();
    assert!(h.app.run("app.about", json!({})).is_ok());
    // (past its slide up: headless time moves a frame at a time)
    for _ in 0..60 {
        h.step();
    }
    let w = widget(&h, "dialog:window").expect("the About window");
    assert!((w.width() - PHONE[0]).abs() < 1.0, "the width of the screen: {w:?}");
    assert!((w.bottom() - PHONE[1]).abs() < 1.0, "down to the bottom edge: {w:?}");
    assert!(widget(&h, "button:aboutTab-contributors").is_none(), "no desktop tabs");
    assert!(widget(&h, "button:dialogOk").is_none(), "no Close button at the bottom");
    assert!(widget(&h, "button:aboutBack").is_none(), "no ‹ on its first page");
    // Contributors is a page, ‹ comes back
    let row = widget(&h, "about:contributors").expect("Contributors as a row");
    assert!(row.height() >= 44.0 && row.width() > PHONE[0] * 0.8, "{row:?}");
    click(&mut h, "about:contributors");
    assert!(widget(&h, "about:contributors").is_none() && widget(&h, "button:aboutBack").is_some(), "Contributors' page");
    let row = widget(&h, "credit:echelon").or_else(|| h.app.widgets.iter().find(|(w, _)| w.starts_with("credit:")).map(|(_, r)| *r));
    assert!(row.is_some_and(|r| r.height() >= 44.0 && r.width() > PHONE[0] * 0.8), "each contributor a row: {row:?}");
    assert!(widget(&h, "button:creditsTable").is_none(), "no desktop table");
    click(&mut h, "button:aboutBack");
    assert!(widget(&h, "about:contributors").is_some(), "back on the first page");
    // What's New and System Info are pages in the sheet, so ‹ comes back to About
    for page in ["about:app.whatsNew", "about:app.systemInfo"] {
        click(&mut h, page);
        assert!(matches!(h.app.ui.dialog, Some(crate::state::Dialog::About)), "{page}: still About");
        assert!(widget(&h, "button:aboutBack").is_some(), "{page}: a page with ‹");
        click(&mut h, "button:aboutBack");
        assert!(widget(&h, "about:contributors").is_some(), "{page}: back on About");
    }
    // System Info: what a phone's owner reads, and the rest copied for a report
    click(&mut h, "about:app.systemInfo");
    assert!(widget(&h, "sysinfo:Version").is_some() && widget(&h, "sysinfo:Photos").is_some());
    assert!(widget(&h, "sysinfo:Frame time").is_none() && widget(&h, "sysinfo:Memory budget").is_none(), "no performance figures");
    assert!(widget(&h, "button:sysinfoCopy").is_some(), "Copy Details");
    click(&mut h, "button:aboutClose");
    assert!(h.app.ui.dialog.is_none(), "✕ closes it");
}

#[test]
fn phone_culling_is_a_sheet_with_the_systems_sliders() {
    let mut h = grid_phone();
    assert!(h.app.run("dialog.cull", json!({})).is_ok());
    for _ in 0..60 {
        h.step();
    }
    let w = widget(&h, "dialog:window").expect("the sheet");
    assert!((w.width() - PHONE[0]).abs() < 1.0 && (w.bottom() - PHONE[1]).abs() < 1.0, "a sheet: {w:?}");
    for id in ["slider:cullReject", "slider:cullGap", "slider:cullSimilar"] {
        let r = widget(&h, id).unwrap_or_else(|| panic!("{id}"));
        assert!(r.width() > PHONE[0] * 0.8, "{id} the width of the sheet: {r:?}");
    }
    assert!(widget(&h, "check:cullPick").is_some_and(|r| r.height() >= 44.0), "a switch row");
    assert!(widget(&h, "button:dialogCancel").is_none() && widget(&h, "button:sheetClose").is_some(), "✕ in place of Cancel");
    let ok = widget(&h, "button:dialogOk").expect("the confirm");
    assert!(ok.width() > PHONE[0] * 0.8 && ok.bottom() > PHONE[1] - 120.0, "full width at the foot: {ok:?}");
    click(&mut h, "button:sheetClose");
    assert!(h.app.ui.dialog.is_none(), "✕ closes it");
}

#[test]
fn phone_face_model_terms_are_a_sheet_that_downloads_once_accepted() {
    let mut h = grid_phone();
    let list = h.app.session.execute("faces.models.list", &json!({})).unwrap_or_default();
    let m = list["models"].as_array().and_then(|a| a.iter().find(|m| m["role"] == "detector").cloned()).expect("the detector");
    crate::dialogs::faces::open_download_dialog(&mut h.app, &m);
    for _ in 0..60 {
        h.step();
    }
    let w = widget(&h, "dialog:window").expect("the sheet");
    assert!((w.width() - PHONE[0]).abs() < 1.0 && (w.bottom() - PHONE[1]).abs() < 1.0, "a sheet: {w:?}");
    assert!(widget(&h, "faceModel:page").is_some_and(|r| r.height() >= 44.0), "the model's page as a row");
    let accept = widget(&h, "check:faceModel.accept").expect("the consent switch");
    assert!(accept.height() >= 44.0, "{accept:?}");
    assert!(widget(&h, "button:dialogOk").is_some(), "Download at the foot");
    click(&mut h, "check:faceModel.accept");
    assert!(matches!(h.app.ui.dialog, Some(crate::state::Dialog::FaceModel { accepted: true, .. })), "accepted");
}

#[test]
fn phone_faces_lists_models_as_rows_with_a_page_each() {
    let dir = std::env::temp_dir().join(format!("lc-phone-faces-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let services = Services { png: None, mobile: true, ..Default::default() };
    let mut h = Headless::new(LightcraftApp::new(lightcraft_engine::Session::with_demo(), services), PHONE, 1.0);
    h.app.session.face_models_dir = Some(dir.join("models"));
    assert_eq!(h.request("ui.set", json!({"view": "photoGrid", "right": "none"}), T)["ok"], true);
    h.settle(SETTLE);
    assert!(h.app.run("app.settings", json!({"tab": "faces"})).is_ok());
    for _ in 0..60 {
        h.step();
    }
    let row = widget(&h, "faces:model:sface-2021dec").expect("SFace as a row");
    assert!(row.height() >= 44.0 && row.width() > PHONE[0] * 0.8, "{row:?}");
    assert!(!any_widget(&h, "faces:download:"), "no buttons in the list");
    click(&mut h, "faces:model:sface-2021dec");
    assert!(widget(&h, "button:settingsBack").is_some(), "its page, with ‹");
    let dl = widget(&h, "button:faces:download:sface-2021dec").expect("Download on its page");
    assert!(dl.height() >= 44.0 && dl.width() > PHONE[0] * 0.8, "the system's button: {dl:?}");
    click(&mut h, "button:faces:download:sface-2021dec");
    assert!(matches!(h.app.ui.dialog, Some(crate::state::Dialog::FaceModel { .. })), "its terms first");
    for _ in 0..60 {
        h.step();
    }
    let tick = widget(&h, "check:faceModel.accept").expect("the consent");
    assert!(tick.height() >= 44.0, "{tick:?}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn system_back_steps_back_then_says_there_is_nowhere_to_go() {
    use crate::panels::back::system_back;
    let mut h = grid_phone();
    let ctx = h.view.ctx.clone();
    // a sheet closes
    click(&mut h, "icon:more");
    assert!(crate::panels::more_sheet::is_open(&ctx));
    assert!(system_back(&mut h.app, &ctx));
    assert!(!crate::panels::more_sheet::is_open(&ctx), "the sheet closed");
    // Settings: a section's page, then the list, then closed
    assert!(h.app.run("app.settings", json!({"tab": "general"})).is_ok());
    for _ in 0..4 {
        h.step();
    }
    click(&mut h, "button:settingsTab-import");
    assert!(system_back(&mut h.app, &ctx));
    h.step();
    assert!(widget(&h, "button:settingsTab-general").is_some(), "back on the list of sections");
    assert!(system_back(&mut h.app, &ctx));
    assert!(h.app.ui.dialog.is_none(), "then Settings closes");
    // the editor goes back to the photos, and the photos have nowhere to go
    assert_eq!(h.request("ui.set", json!({"view": "detail"}), T)["ok"], true);
    h.step();
    assert!(system_back(&mut h.app, &ctx));
    assert_eq!(h.app.ui.view, ViewMode::PhotoGrid);
    h.step();
    assert!(!system_back(&mut h.app, &ctx), "nothing left: the host leaves the app");
}

#[test]
fn people_goes_back_to_the_photos_and_the_back_key_steps_back() {
    let mut h = grid_phone();
    // ‹ in People's bar (and Esc) went nowhere: People had no step back
    assert!(h.app.run("view.people", json!({})).is_ok());
    h.step();
    assert_eq!(h.app.ui.view, ViewMode::People);
    assert!(h.app.run("view.back", json!({})).is_ok());
    assert_eq!(h.app.ui.view, ViewMode::PhotoGrid, "People goes back to the photos");
    // Android's Back reaches the UI as the browser's Back key
    let back =
        || egui::Event::Key { key: egui::Key::BrowserBack, physical_key: None, pressed: true, repeat: false, modifiers: egui::Modifiers::NONE };
    assert!(h.app.run("view.people", json!({})).is_ok());
    h.input([back()]);
    h.step();
    assert_eq!(h.app.ui.view, ViewMode::PhotoGrid, "the key steps back too");
    assert!(!crate::panels::back::take_leave(), "somewhere to go: the app stays");
    h.input([back()]);
    h.step();
    assert!(crate::panels::back::take_leave(), "nowhere left: the host is asked to leave");
}

#[test]
fn phone_interface_is_a_phones_display_settings() {
    let mut h = mobile_settings("interface");
    // Light and Dark as phones side by side, Automatic, then each family as a ticked list
    assert!(widget(&h, "button:appearanceLight").is_some() && widget(&h, "button:appearanceDark").is_some());
    assert!(widget(&h, "check:settings.appearanceAuto").is_some(), "Automatic");
    assert!(widget(&h, "button:themeLight-0").is_some() && widget(&h, "button:themeDark-1").is_some());
    assert!(!h.app.widgets.iter().any(|(w, _)| w.starts_with("radio:theme-") || w.starts_with("button:themeFavorite-")), "no desktop cards");
    click(&mut h, "button:appearanceLight");
    assert_eq!(h.app.ui.settings.appearance_mode, crate::state::AppearanceMode::Light);
    click(&mut h, "check:settings.appearanceAuto");
    assert_eq!(h.app.ui.settings.appearance_mode, crate::state::AppearanceMode::Auto);
    // the other choices: full-width rows with a tick
    let page = widget(&h, "settings:page").expect("the page");
    let row = widget(&h, "button:settingsGridBadges-0").expect("Ratings & flags as a list");
    assert!(row.height() >= 44.0 && row.width() > page.width() - 40.0, "{row:?} in {page:?}");
}

#[test]
fn phone_language_opens_its_own_page() {
    let mut h = mobile_settings("general");
    click(&mut h, "button:settingsTab-general");
    click(&mut h, "combo:settingsLanguage");
    // the languages take the page, as a phone's settings push one
    assert!(widget(&h, "settings:subpage").is_some(), "a page of its own");
    assert!(widget(&h, "combo:settingsLanguage").is_none() && widget(&h, "settings:page").is_none(), "in place of General");
    let languages = crate::i18n::Locale::for_picker();
    let fr = languages.iter().position(|l| *l == crate::i18n::Locale::Fr).expect("French is offered");
    let row = widget(&h, &format!("button:settingsLanguageList-{fr}")).expect("its row");
    assert!(row.height() >= 44.0, "{row:?}");
    // ‹ goes back to General without a change
    click(&mut h, "button:settingsBack");
    assert!(widget(&h, "combo:settingsLanguage").is_some(), "back on General");
    assert_eq!(h.app.ui.language, crate::i18n::Locale::En);
    // a pick goes back too, and takes effect
    click(&mut h, "combo:settingsLanguage");
    click(&mut h, &format!("button:settingsLanguageList-{fr}"));
    h.step();
    assert_eq!(h.app.ui.language, crate::i18n::Locale::Fr);
    assert!(widget(&h, "settings:subpage").is_none() && widget(&h, "combo:settingsLanguage").is_some(), "back on General");
    h.app.ui.language = crate::i18n::Locale::En;
    crate::i18n::set_language(crate::i18n::Locale::En);
}

#[test]
fn phone_cache_slider_is_the_systems_kind() {
    let mut h = mobile_settings("performance");
    let r = widget(&h, "slider:settingsCache").expect("the slider");
    let page = widget(&h, "settings:page").expect("the page");
    assert!(r.width() > page.width() - 20.0 && r.height() >= 40.0, "the width of the page, a finger tall: {r:?}");
    // a tap at the far right sets the most, applied at once
    let at = egui::pos2(r.right() - 2.0, r.center().y);
    let res = h.request("ui.click", json!({"x": at.x, "y": at.y}), T);
    assert_eq!(res["ok"], true, "{res}");
    for _ in 0..4 {
        h.step();
    }
    assert_eq!(h.app.session.cache_bytes() >> 20, 8192);
}

#[test]
fn phone_settings_leave_out_ai_denoise_and_large_face_models() {
    let h = mobile_settings("general");
    assert!(widget(&h, "button:settingsTab-faces").is_some(), "Faces stays");
    assert!(widget(&h, "button:settingsTab-denoise").is_none(), "no AI Denoise");
    let h = mobile_settings("faces");
    let ids: Vec<&str> = h.app.widgets.iter().map(|(w, _)| w.as_str()).collect();
    assert!(!ids.iter().any(|w| w.ends_with(":auraface-v1")), "no AuraFace: {ids:?}");
    assert!(!ids.contains(&"faces:addModel") && !ids.iter().any(|w| w.starts_with("faces:device:")), "no model files or processors");
    // a link to AI Denoise shows General
    let h = mobile_settings("denoise");
    assert!(widget(&h, "combo:settingsLanguage").is_some(), "General instead");
}

#[test]
fn phone_about_holds_the_help_menus_rows() {
    let mut h = grid_phone();
    assert!(h.app.run("app.about", json!({})).is_ok());
    for _ in 0..4 {
        h.step();
    }
    for id in ["about:app.whatsNew", "about:app.feedback", "about:app.help", "about:app.systemInfo"] {
        let r = widget(&h, id).unwrap_or_else(|| panic!("{id} in About"));
        assert!(r.height() >= 44.0, "{id}: {r:?}");
    }
}

/// A phone in the editor, with a PNG encoder when `$LIGHTCRAFT_TEST_SHOTS` asks for screenshots.
fn editor_phone_for_shots() -> Headless {
    let shots = std::env::var_os("LIGHTCRAFT_TEST_SHOTS").is_some();
    let png: Option<crate::PngEncode> = shots.then(|| -> crate::PngEncode {
        Box::new(|img: &lightcraft_raster::Rgba8| {
            lightcraft_codecs::encode_png(&lightcraft_codecs::EncodeImage::rgba8(img), &lightcraft_codecs::EncodeMeta::default()).unwrap_or_default()
        })
    });
    let services = Services {
        png,
        write: Some(Box::new(lightcraft_engine::export::write_file)),
        write_new: Some(Box::new(lightcraft_engine::export::write_file_new)),
        ..Default::default()
    };
    let mut h = Headless::new(LightcraftApp::new(lightcraft_engine::Session::with_demo(), services), PHONE, if shots { 3.0 } else { 1.0 });
    assert_eq!(h.request("ui.set", json!({"view": "detail", "right": "edit"}), T)["ok"], true);
    h.settle(SETTLE);
    h
}

/// A headless screenshot into `$LIGHTCRAFT_TEST_SHOTS`, when it is set.
fn shot(h: &mut Headless, name: &str) {
    if let Some(p) = std::env::var_os("LIGHTCRAFT_TEST_SHOTS") {
        let r = h.request("ui.screenshot", json!({"path": std::path::Path::new(&p).join(name).to_string_lossy(), "headless": true}), T);
        assert_eq!(r["ok"], true, "{r}");
    }
}

#[test]
fn phone_copy_settings_is_a_sheet_of_tick_boxes_with_copy_at_its_foot() {
    use crate::state::Dialog;
    let mut h = editor_phone_for_shots();
    assert!(h.app.run("dialog.copySettings", json!({})).is_ok());
    for _ in 0..60 {
        h.step();
    }
    shot(&mut h, "copy-settings.png");
    let close = widget(&h, "button:sheetClose").expect("the sheet's ✕");
    let all = widget(&h, "check:copyGroup:all").expect("Select All");
    let light = widget(&h, "check:copyGroup:light").expect("Light as a row");
    assert!(light.height() >= 44.0 && light.width() > PHONE[0] * 0.8, "a row a finger can take: {light:?}");
    assert!(close.bottom() <= all.top() && all.bottom() <= light.top());
    let ok = widget(&h, "button:dialogOk").expect("Copy at the foot");
    assert!(ok.width() > PHONE[0] * 0.8 && ok.bottom() > PHONE[1] - 80.0, "{ok:?}");
    // a row's tap ticks it off, Select All ticks them all
    let had = |h: &Headless, k: &str| matches!(&h.app.ui.dialog, Some(Dialog::CopySettings { groups }) if groups.iter().any(|g| g == k));
    assert!(had(&h, "light"));
    click(&mut h, "check:copyGroup:light");
    assert!(!had(&h, "light"), "unticked");
    click(&mut h, "check:copyGroup:all");
    assert!(had(&h, "light") && had(&h, "crop"), "all ticked");
    click(&mut h, "button:dialogOk");
    assert!(h.app.ui.dialog.is_none(), "copied, and closed");
}

#[test]
fn phone_create_preset_is_a_sheet_and_presets_plus_opens_a_menu() {
    let mut h = editor_phone_for_shots();
    click(&mut h, "icon:presets");
    assert!(widget(&h, "icon:presetMore").is_none(), "no \"…\" on a phone");
    click(&mut h, "icon:presetAdd");
    shot(&mut h, "preset-add-menu.png");
    for id in ["presetAddMenu:dialog.createPreset", "presetAddMenu:file.importPresets", "presetAddMenu:file.exportPresets"] {
        assert!(widget(&h, id).is_some(), "{id} in +'s menu");
    }
    click(&mut h, "presetAddMenu:dialog.createPreset");
    assert!(matches!(h.app.ui.dialog, Some(crate::state::Dialog::CreatePreset { .. })));
    for _ in 0..60 {
        h.step();
    }
    shot(&mut h, "create-preset.png");
    assert!(widget(&h, "field:presetName").is_some() && widget(&h, "check:presetInclude:light").is_some());
    assert!(widget(&h, "button:dialogOk").is_some_and(|ok| ok.width() > PHONE[0] * 0.8), "Create at the foot");
}

#[test]
fn presets_plus_is_the_hosts_own_menu_where_it_has_one() {
    let placed = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let seen = placed.clone();
    let services = Services {
        menu_button: Some(Box::new(move |r: crate::native_menu::MenuRequest| {
            *seen.borrow_mut() = r.iter().map(|b| (b.key, b.items.len())).collect()
        })),
        ..Default::default()
    };
    let mut h = Headless::new(LightcraftApp::new(lightcraft_engine::Session::with_demo(), services), PHONE, 1.0);
    assert_eq!(h.request("ui.set", json!({"view": "detail", "right": "edit"}), T)["ok"], true);
    h.settle(SETTLE);
    click(&mut h, "icon:presets");
    let keys: Vec<&str> = placed.borrow().iter().map(|(k, _)| *k).collect();
    assert!(keys.contains(&"more") && keys.contains(&"presetAdd"), "{keys:?}");
    assert!(placed.borrow().iter().any(|(k, n)| *k == "presetAdd" && *n == 3));
}

#[test]
fn phone_keeps_desktop_things_out_of_its_drawer_menus_and_tools() {
    let services = Services { png: None, mobile: true, ..Default::default() };
    let mut h = Headless::new(LightcraftApp::new(lightcraft_engine::Session::with_demo(), services), PHONE, 1.0);
    assert_eq!(h.request("ui.set", json!({"view": "photoGrid", "right": "none"}), T)["ok"], true);
    h.settle(SETTLE);
    // the drawer: no home folder, Desktop or Downloads to browse
    click(&mut h, "icon:sidebar");
    assert!(widget(&h, "source:all").is_some(), "the drawer is open");
    let ids: Vec<&String> = h.app.widgets.iter().map(|(w, _)| w).collect();
    assert!(ids.iter().any(|w| w.starts_with("source:album:")), "the drawer's albums");
    assert!(!ids.iter().any(|w| w.contains("ocal")), "no Local places on a phone: {ids:?}");
    // a photo's long-press menu: no other editors, no Reload from Disk, no Reference View
    let ctx = h.view.ctx.clone();
    crate::panels::back::system_back(&mut h.app, &ctx);
    h.step();
    let id = h.app.session.visible_shared().1[0];
    let _ = h.request("ui.clickWidget", json!({"id": format!("thumb:{}", id.0), "button": "right"}), T);
    h.step();
    h.step();
    let shown = |h: &Headless, text: &str| h.view.painted_shapes().iter().any(|s| matches!(s, egui::Shape::Text(t) if t.galley.job.text == text));
    assert!(shown(&h, "Find Similar Photos"), "the menu is there");
    for gone in ["Edit In", "Reload from Disk", "Set as Reference Photo"] {
        assert!(!shown(&h, gone), "{gone} on a phone");
    }
    // a tool opened from the row: no "Masking On" toast over the photo
    h.app.ui.toast = None;
    assert!(h.app.run("panel.masking", json!({})).is_ok());
    assert!(h.app.ui.toast.is_none(), "{:?}", h.app.ui.toast);
}

#[test]
fn phone_crop_is_chips_and_round_tools() {
    let mut h = demo(PHONE, json!({"view": "detail", "right": "edit"}));
    // the tool row's Crop, wherever the row is scrolled
    let ctx = h.view.ctx.clone();
    crate::panels::compact::press_tab(&mut h.app, &ctx, "crop");
    h.step();
    h.step();
    let chip = widget(&h, "button:cropAspect-1x1").expect("1 × 1 as a chip");
    assert!(chip.height() >= 36.0, "{chip:?}");
    for id in
        ["button:cropRotateLeft", "button:cropFlipH", "button:cropFlipV", "button:cropSwapAspect", "button:straightenTool", "button:straightenAuto"]
    {
        let r = widget(&h, id).unwrap_or_else(|| panic!("{id}"));
        assert!(r.height() >= 44.0, "{id}: {r:?}");
    }
    let shown =
        |h: &Headless, text: &str| h.view.painted_shapes().iter().any(|s| matches!(s, egui::Shape::Text(t) if t.galley.job.text.contains(text)));
    assert!(!shown(&h, "(X)") && !shown(&h, "Locked"), "no key hints or Lock button on a phone");
    click(&mut h, "button:cropAspect-1x1");
    assert_eq!(h.app.crop_tool_aspect(), Some((100, 100)), "a chip locks its aspect");
    click(&mut h, "button:cropAspect-free");
    assert_eq!(h.app.crop_tool_aspect(), None, "Free unlocks");
    let flipped = |h: &Headless| h.app.session.active().and_then(|id| h.app.session.develop_of(id)).is_some_and(|d| d.crop.flip_h);
    click(&mut h, "button:cropFlipH");
    assert!(flipped(&h), "Flip H flips");
}

#[test]
fn phone_long_press_on_the_photo_shows_the_original_not_a_menu() {
    let mut h = demo(PHONE, json!({"view": "detail", "right": "edit"}));
    touch_screen(&mut h);
    let photo = h.app.canvas_rect.expect("the photo's room").center();
    let before = h.app.ui.before_after;
    hold(&mut h, photo, 60, |h| {
        assert_eq!(h.app.ui.before_after, crate::state::BeforeAfter::Original, "the photo as shot while held");
        assert!(widget(h, "panel:contextSheet").is_none(), "no menu under the finger");
    });
    assert_eq!(h.app.ui.before_after, before, "back once the finger lifts");
    assert!(widget(&h, "panel:contextSheet").is_none());
}

#[test]
fn phone_rename_is_a_name_field_for_one_photo_and_formats_for_more() {
    use crate::state::Dialog;
    let mut h = editor_phone_for_shots();
    let template = |h: &Headless| match &h.app.ui.dialog {
        Some(Dialog::Rename { template, .. }) => template.clone(),
        _ => String::new(),
    };
    // one photo: its name, ready to change
    assert!(h.app.run("dialog.rename", json!({})).is_ok());
    for _ in 0..60 {
        h.step();
    }
    shot(&mut h, "rename-one.png");
    assert!(widget(&h, "field:renameTemplate").is_some(), "a Name field");
    let name = template(&h);
    assert!(!name.is_empty() && !name.contains('{'), "it holds the photo's name: {name}");
    assert!(widget(&h, "button:renamePreset-0").is_none(), "no formats for one photo");
    h.app.ui.dialog = None;
    h.step();
    // several: name formats as a list, the template only under Custom
    let _ = h.request("engine.execute", json!({"command": "library.selectAll", "params": {}}), T);
    assert!(h.app.run("dialog.rename", json!({})).is_ok());
    for _ in 0..60 {
        h.step();
    }
    shot(&mut h, "rename-many.png");
    let row = widget(&h, "button:renamePreset-1").expect("Date and Name as a row");
    assert!(row.height() >= 44.0 && row.width() > PHONE[0] * 0.8, "{row:?}");
    assert!(widget(&h, "field:renameTemplate").is_none(), "no template until Custom");
    click(&mut h, "button:renamePreset-custom");
    assert!(widget(&h, "field:renameTemplate").is_some(), "Custom shows the template");
    assert!(template(&h).contains("{seq"), "{}", template(&h));
    let start = |h: &Headless| match &h.app.ui.dialog {
        Some(Dialog::Rename { start, .. }) => *start,
        _ => 0,
    };
    let was = start(&h);
    click(&mut h, "button:renameStartUp");
    assert_eq!(start(&h), was + 1, "the stepper counts up");
}
