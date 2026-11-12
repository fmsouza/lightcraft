//! iOS: the system's haptics for the app's knocks ([`lightcraft_ui_egui::Haptic`]): a medium
//! impact when a long press takes effect, the selection tick Photos gives as a slide selects.
//! The generators are made once, on the main thread, where the UI asks for them.

use std::cell::RefCell;

use lightcraft_ui_egui::Haptic;
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2_ui_kit::{UIImpactFeedbackGenerator, UIImpactFeedbackStyle, UISelectionFeedbackGenerator};

type Generators = (Retained<UIImpactFeedbackGenerator>, Retained<UISelectionFeedbackGenerator>);

thread_local! {
    static GENERATORS: RefCell<Option<Generators>> = const { RefCell::new(None) };
}

/// Knock for `kind` (nothing off the main thread).
// `initWithStyle:` is deprecated from iOS 17.5 for `feedbackGeneratorWithStyle:forView:`, which
// iOS 17.0 (the app's minimum) doesn't have; the old one still works everywhere.
#[allow(deprecated)]
pub fn knock(kind: Haptic) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    GENERATORS.with(|g| {
        let mut g = g.borrow_mut();
        let (press, tick) = g.get_or_insert_with(|| {
            (UIImpactFeedbackGenerator::initWithStyle(mtm.alloc(), UIImpactFeedbackStyle::Medium), UISelectionFeedbackGenerator::new(mtm))
        });
        match kind {
            Haptic::Press => press.impactOccurred(),
            Haptic::Tick => tick.selectionChanged(),
        }
    });
}
