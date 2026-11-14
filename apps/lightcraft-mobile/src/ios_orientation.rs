//! iOS: which way the phone is held. The app's interface stays upright on an iPhone (Info.plist
//! lists only portrait), so UIKit does not turn it; the editor still shows the photo alone, turned
//! to fill the screen, when the phone is held sideways (`LightcraftApp::device_turn`). UIDevice
//! reports the physical orientation once asked to generate it; a change wakes the app through
//! its notification, so nothing polls.
#![allow(unsafe_code)] // Objective-C messages and a notification block

use std::sync::Once;
use std::sync::atomic::{AtomicI8, Ordering};

use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2_foundation::{NSNotification, NSNotificationCenter, NSOperationQueue};
use objc2_ui_kit::{UIDevice, UIDeviceOrientation, UIDeviceOrientationDidChangeNotification, UIUserInterfaceIdiom};

/// 1: turned with its top to the left (UIKit's landscape left), -1: to the right, 0: upright.
static TURN: AtomicI8 = AtomicI8::new(0);
static STARTED: Once = Once::new();

/// The turn for an orientation; `None` (flat on a table, unknown) keeps the last one.
fn turn_of(o: UIDeviceOrientation) -> Option<i8> {
    match o {
        UIDeviceOrientation::LandscapeLeft => Some(1),
        UIDeviceOrientation::LandscapeRight => Some(-1),
        UIDeviceOrientation::Portrait | UIDeviceOrientation::PortraitUpsideDown => Some(0),
        _ => None,
    }
}

/// Start following the phone's orientation (once; on the main thread), repainting `ctx` on a change.
pub fn start(ctx: &egui::Context) {
    let Some(mtm) = MainThreadMarker::new() else { return };
    STARTED.call_once(|| {
        let device = UIDevice::currentDevice(mtm);
        // an iPad's interface turns with it (all four orientations): only a phone's stays upright
        if device.userInterfaceIdiom() != UIUserInterfaceIdiom::Phone {
            return;
        }
        device.beginGeneratingDeviceOrientationNotifications();
        if let Some(t) = turn_of(device.orientation()) {
            TURN.store(t, Ordering::Relaxed);
        }
        let ctx = ctx.clone();
        let block = RcBlock::new(move |_: std::ptr::NonNull<NSNotification>| {
            // (delivered on the main queue, given below)
            if let Some(mtm) = MainThreadMarker::new()
                && let Some(t) = turn_of(UIDevice::currentDevice(mtm).orientation())
            {
                TURN.store(t, Ordering::Relaxed);
                ctx.request_repaint();
            }
        });
        let queue = NSOperationQueue::mainQueue();
        // SAFETY: the system's notification name and main queue; the block only reads UIDevice on
        // the main thread and asks egui (thread-safe) to repaint
        let observer = unsafe {
            NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(
                Some(UIDeviceOrientationDidChangeNotification),
                None,
                Some(&queue),
                &block,
            )
        };
        // the observer lives as long as the app
        std::mem::forget(observer);
    });
}

/// Which way the phone is held: 1 (top to the left), -1 (top to the right) or 0 (upright).
pub fn turn() -> i8 {
    TURN.load(Ordering::Relaxed)
}
