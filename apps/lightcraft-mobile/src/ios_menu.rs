//! iOS: the phone's "…" (and other menu buttons, Presets' +) as the system's menu. A clear UIButton
//! sits over each button egui drew and takes its tap; it opens a UIMenu (sections with titles, SF
//! Symbols, check marks, submenus) built when it opens from the latest items the app sent
//! ([`lightcraft_ui_egui::native_menu`]).
//! What is picked goes back to the app, which runs it on the next frame.
#![allow(unsafe_code)] // Objective-C messages and blocks

use std::cell::RefCell;
use std::collections::HashMap;
use std::ptr::NonNull;
use std::sync::Arc;

use block2::{DynBlock, RcBlock};
use lightcraft_ui_egui::native_menu::{MenuButton, MenuRequest, NativeItem};
use objc2::rc::Retained;
use objc2::{MainThreadMarker, msg_send};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::{NSArray, NSString};
use objc2_ui_kit::{
    UIAction, UIButton, UIButtonType, UIDeferredMenuElement, UIImage, UIMenu, UIMenuElement, UIMenuElementAttributes, UIMenuElementState,
    UIMenuOptions,
};

thread_local! {
    /// A native button for each of the app's menu buttons, by key (made once, hidden when off screen).
    static BUTTONS: RefCell<HashMap<&'static str, Retained<UIButton>>> = RefCell::new(HashMap::new());
    /// For each button on screen: where it is (egui points) and the items its menu shows when it
    /// next opens.
    static LATEST: RefCell<HashMap<&'static str, (egui::Rect, Arc<Vec<NativeItem>>)>> = RefCell::new(HashMap::new());
    /// The last request carried out in full (every button made): the same one again changes nothing.
    static DONE: RefCell<Option<MenuRequest>> = const { RefCell::new(None) };
}

/// The app's request for this frame (main thread): move each native button over its button, and
/// hide the ones whose buttons aren't on screen.
pub fn update(request: MenuRequest, ctx: &egui::Context) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    // most frames ask for what the last one did: nothing to move, hide or hand over
    if DONE.with_borrow(|d| d.as_ref() == Some(&request)) {
        return;
    }
    BUTTONS.with_borrow(|buttons| {
        for (key, button) in buttons {
            if !request.iter().any(|r| r.key == *key) {
                button.setHidden(true);
            }
        }
    });
    LATEST.with_borrow_mut(|l| l.retain(|key, _| request.iter().any(|r| r.key == *key)));
    for r in &request {
        place(mtm, r.clone(), ctx);
    }
    // a button not made yet (no view to put it on) is tried again next frame
    let all_made = BUTTONS.with_borrow(|b| request.iter().all(|r| b.contains_key(r.key)));
    DONE.set(all_made.then_some(request));
}

/// Put `r`'s native button over it (made the first time).
fn place(mtm: MainThreadMarker, r: MenuButton, ctx: &egui::Context) {
    let rect = r.rect;
    let moved = LATEST.with_borrow(|l| l.get(r.key).map(|(at, _)| *at) != Some(rect));
    LATEST.with_borrow_mut(|l| l.insert(r.key, (rect, r.items)));
    let button = BUTTONS.with_borrow_mut(|b| {
        if !b.contains_key(r.key) {
            let made = make_button(mtm, ctx, r.key, &r.label)?;
            b.insert(r.key, made);
        }
        b.get(r.key).cloned()
    });
    let Some(button) = button else { return };
    if moved || button.isHidden() {
        // egui points → UIKit points
        let ppp = ctx.pixels_per_point();
        let scale = f64::from(ppp / ctx.native_pixels_per_point().filter(|n| *n > 0.0).unwrap_or(ppp));
        if !scale.is_finite() {
            return;
        }
        button.setFrame(CGRect::new(
            CGPoint::new(f64::from(rect.left()) * scale, f64::from(rect.top()) * scale),
            CGSize::new(f64::from(rect.width()) * scale, f64::from(rect.height()) * scale),
        ));
        button.setHidden(false);
    }
}

/// The clear button over the app's button `key`, on winit's view, with a menu whose items are
/// made as it opens.
fn make_button(mtm: MainThreadMarker, ctx: &egui::Context, key: &'static str, label: &str) -> Option<Retained<UIButton>> {
    let view = crate::ios_scene::root_view_controller()?.view()?;
    let button = UIButton::buttonWithType(UIButtonType::System, mtm);
    // SAFETY: setting the accessibility label NSString property of a UIView, on the main thread
    let _: () = unsafe { msg_send![&*button, setAccessibilityLabel: &*NSString::from_str(label)] };
    let ctx = ctx.clone();
    let provider = RcBlock::new(move |done: NonNull<DynBlock<dyn Fn(NonNull<NSArray<UIMenuElement>>)>>| {
        let items = LATEST.with_borrow(|l| l.get(key).map(|(_, i)| i.clone())).unwrap_or_default();
        let elements = elements(mtm, &items, &ctx);
        // SAFETY: UIKit's completion block, called once on the main thread with the menu's items
        unsafe { done.as_ref() }.call((NonNull::from(&*elements),));
    });
    // SAFETY: an uncached provider: UIKit calls it each time the menu opens, on the main thread
    let deferred = unsafe { UIDeferredMenuElement::elementWithUncachedProvider(&provider, mtm) };
    let children = NSArray::from_retained_slice(&[Retained::into_super(deferred)]);
    let menu = UIMenu::menuWithTitle_image_identifier_options_children(&NSString::from_str(""), None, None, UIMenuOptions::empty(), &children, mtm);
    button.setMenu(Some(&menu));
    button.setShowsMenuAsPrimaryAction(true);
    button.setHidden(true);
    view.addSubview(&button);
    Some(button)
}

/// `items` as UIKit menu elements.
fn elements(mtm: MainThreadMarker, items: &[NativeItem], ctx: &egui::Context) -> Retained<NSArray<UIMenuElement>> {
    let list: Vec<Retained<UIMenuElement>> = items.iter().map(|i| element(mtm, i, ctx)).collect();
    NSArray::from_retained_slice(&list)
}

fn symbol(name: &str) -> Option<Retained<UIImage>> {
    (!name.is_empty()).then(|| UIImage::systemImageNamed(&NSString::from_str(name))).flatten()
}

fn element(mtm: MainThreadMarker, item: &NativeItem, ctx: &egui::Context) -> Retained<UIMenuElement> {
    match item {
        NativeItem::Action { id, params, label, symbol: sym, enabled, checked, destructive } => {
            let (id, params, ctx) = (id.clone(), params.clone(), ctx.clone());
            let handler = RcBlock::new(move |_: NonNull<UIAction>| {
                lightcraft_ui_egui::native_menu::pick(&id, params.clone());
                ctx.request_repaint();
            });
            let image = symbol(sym);
            // SAFETY: a live block (UIKit copies it) and a title, on the main thread
            let action = unsafe {
                UIAction::actionWithTitle_image_identifier_handler(&NSString::from_str(label), image.as_deref(), None, RcBlock::as_ptr(&handler), mtm)
            };
            let mut attributes = UIMenuElementAttributes::empty();
            if !enabled {
                attributes |= UIMenuElementAttributes::Disabled;
            }
            if *destructive {
                attributes |= UIMenuElementAttributes::Destructive;
            }
            action.setAttributes(attributes);
            if *checked {
                action.setState(UIMenuElementState::On);
            }
            Retained::into_super(action)
        }
        NativeItem::Section { title, items } => Retained::into_super(UIMenu::menuWithTitle_image_identifier_options_children(
            &NSString::from_str(title),
            None,
            None,
            UIMenuOptions::DisplayInline,
            &elements(mtm, items, ctx),
            mtm,
        )),
        NativeItem::Submenu { label, symbol: sym, items } => Retained::into_super(UIMenu::menuWithTitle_image_identifier_options_children(
            &NSString::from_str(label),
            symbol(sym).as_deref(),
            None,
            UIMenuOptions::empty(),
            &elements(mtm, items, ctx),
            mtm,
        )),
    }
}
