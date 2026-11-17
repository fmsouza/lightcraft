//! iOS: save exported files to the photo library (the Share sheet's Save to Photos).
//!
//! One PhotoKit change per batch: a creation request per file, added as the asset's photo
//! resource from its file (so DNG, TIFF, AVIF and JPEG XL go in as they are, which iOS reads).
//! The first save asks for add-only access (`NSPhotoLibraryAddUsageDescription`). PhotoKit answers
//! on a queue of its own; [`take_results`] hands the outcome to the shell, which shows it.
#![allow(unsafe_code)] // Objective-C messages and blocks

use std::sync::Mutex;

use block2::RcBlock;
use objc2::msg_send;
use objc2::runtime::Bool;
use objc2_foundation::{NSError, NSString, NSURL};
use objc2_photos::{PHAssetCreationRequest, PHAssetResourceType, PHPhotoLibrary};

/// Saves that have finished: how many files, or why it failed.
static RESULTS: Mutex<Vec<Result<usize, String>>> = Mutex::new(Vec::new());

/// What the photo library takes: images, not an Original export's XMP sidecars.
fn is_image(path: &str) -> bool {
    let ext = std::path::Path::new(path).extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
    !matches!(ext.as_str(), "xmp" | "pp3" | "json" | "txt")
}

/// Add `paths` to the photo library; the outcome comes from [`take_results`] (`ctx` repaints then).
pub fn save(paths: &[String], ctx: egui::Context) {
    let files: Vec<String> = paths.iter().filter(|p| is_image(p)).cloned().collect();
    if files.is_empty() {
        return;
    }
    let n = files.len();
    let change = RcBlock::new(move || {
        for f in &files {
            let url = NSURL::fileURLWithPath(&NSString::from_str(f));
            // SAFETY: inside a photo library change block, as PhotoKit requires
            unsafe {
                let req = PHAssetCreationRequest::creationRequestForAsset();
                req.addResourceWithType_fileURL_options(PHAssetResourceType::Photo, &url, None);
            }
        }
    });
    let done = RcBlock::new(move |ok: Bool, error: *mut NSError| {
        let r = if ok.as_bool() {
            Ok(n)
        } else {
            // SAFETY: PhotoKit passes a valid NSError (or nil) to the completion handler
            let why = unsafe { error.as_ref() }.map_or_else(|| "unknown error".to_string(), |e| e.localizedDescription().to_string());
            Err(why)
        };
        if let Ok(mut v) = RESULTS.lock() {
            v.push(r);
        }
        ctx.request_repaint();
    });
    // SAFETY: the shared library; the blocks live as long as PhotoKit keeps them (it copies them)
    unsafe {
        let lib = PHPhotoLibrary::sharedPhotoLibrary();
        let _: () = msg_send![&*lib, performChanges: &*change, completionHandler: &*done];
    }
}

/// Saves finished since the last call, oldest first.
pub fn take_results() -> Vec<Result<usize, String>> {
    RESULTS.lock().map(|mut r| std::mem::take(&mut *r)).unwrap_or_default()
}
