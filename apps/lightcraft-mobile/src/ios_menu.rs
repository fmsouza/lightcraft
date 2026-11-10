//! iOS: the phone's "…" as the system's menu. A clear UIButton sits over the "…" egui drew and
//! takes its tap; it opens a UIMenu (sections with titles, SF Symbols, check marks, submenus)
//! built when it opens from the latest items the app sent ([`lightcraft_ui_egui::native_menu`]).
//! What is picked goes back to the app, which runs it on the next frame.
#![allow(unsafe_code)] // Objective-C messages and blocks

use std::cell::RefCell;
use std::ptr::NonNull;
use std::sync::Arc;

use block2::{DynBlock, RcBlock};
use lightcraft_ui_egui::native_menu::{MenuRequest, NativeItem};
use objc2::rc::Retained;
use objc2::{MainThreadMarker, msg_send};
use objc2_core_foundation::{CGPoint, CGRect, CGSize};
use objc2_foundation::{NSArray, NSString};
use objc2_ui_kit::{
    UIAction, UIButton, UIButtonType, UIDeferredMenuElement, UIImage, UIMenu, UIMenuElement, UIMenuElementAttributes, UIMenuElementState,
    UIMenuOptions,
};

thread_local! {
    static BUTTON: RefCell<Option<Retained<UIButton>>> = const { RefCell::new(None) };
    /// The items the menu shows when it next opens, and where the button is (egui points).
    static LATEST: RefCell<Option<(egui::Rect, Arc<Vec<NativeItem>>)>> = const { RefCell::new(None) };
}

/// The app's request for this frame (main thread): move the button over "…", or hide it.
pub fn update(request: MenuRequest, ctx: &egui::Context) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    let rect = request.as_ref().map(|(r, _)| *r);
    let moved = LATEST.with_borrow(|l| l.as_ref().map(|(r, _)| *r) != rect);
    LATEST.with_borrow_mut(|l| *l = request);
    let button = BUTTON.with_borrow_mut(|b| {
        if b.is_none() {
            *b = make_button(mtm, ctx);
        }
        b.clone()
    });
    let Some(button) = button else { return };
    let Some(rect) = rect else {
        button.setHidden(true);
        return;
    };
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

/// The clear button over "…", on winit's view, with a menu whose items are made as it opens.
fn make_button(mtm: MainThreadMarker, ctx: &egui::Context) -> Option<Retained<UIButton>> {
    let view = crate::ios_scene::root_view_controller()?.view()?;
    let button = UIButton::buttonWithType(UIButtonType::System, mtm);
    // SAFETY: setting the accessibility label NSString property of a UIView, on the main thread
    let _: () = unsafe { msg_send![&*button, setAccessibilityLabel: &*NSString::from_str("More")] };
    let ctx = ctx.clone();
    let provider = RcBlock::new(move |done: NonNull<DynBlock<dyn Fn(NonNull<NSArray<UIMenuElement>>)>>| {
        let items = LATEST.with_borrow(|l| l.as_ref().map(|(_, i)| i.clone())).unwrap_or_default();
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
