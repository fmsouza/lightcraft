use super::widgets::{check, choices, heading, hint, row};
#[cfg(not(target_arch = "wasm32"))]
use super::widgets::{label_width, path_label};
use super::{CACHE_SIZES, LABEL_W, LightcraftApp, PREVIEW_LIMITS, RichText, Tokens, Value, json, register};

// --------------------------------------------------------------------------------- Performance

pub(super) fn performance_tab(app: &mut LightcraftApp, ui: &mut egui::Ui, t: &Tokens) {
    // a phone or tablet app: the system's GPU always, a preview at the screen's size, the app's
    // own memory; what is left to choose is the storage the caches take
    if super::mobile(app) {
        mobile_storage(app, ui, t);
        return;
    }
    rendering(app, ui, t);
    heading(ui, t, crate::i18n::tr("Thumbnail cache"));
    let cur = (app.session.cache_bytes() >> 20) as u32;
    row(ui, t, crate::i18n::tr("Size limit"), |ui| {
        let opts = [(512u32, "512 MB"), (1024, "1 GB"), (2048, "2 GB"), (4096, "4 GB"), (8192, "8 GB")];
        let mut v = if CACHE_SIZES.contains(&cur) { cur } else { 2048 };
        if choices(ui, "settingsCache", &opts, &mut v) {
            let _ = app.run("library.preferences", json!({"cacheMb": v}));
        }
    });
    cache_in_use(app, ui, t);
    local_folders(app, ui, t);
    #[cfg(not(target_arch = "wasm32"))]
    smart_previews(app, ui, t);
}

/// The phone or tablet app's Performance page: the thumbnail cache's size limit on a slider, what
/// it holds now (and Clear Cache), and how much the smart previews take.
fn mobile_storage(app: &mut LightcraftApp, ui: &mut egui::Ui, t: &Tokens) {
    let cur = (app.session.cache_bytes() >> 20) as u32;
    // the value being dragged, applied when the finger lets go (not a command a frame)
    let key = egui::Id::unique("settings-cache-slider");
    let mut mb: u32 = ui.data(|d| d.get_temp(key)).unwrap_or(cur).clamp(256, 8192);
    let used = app.session.media.rendered.disk().map(|d| d.size());
    heading(ui, t, crate::i18n::tr("Local storage"));
    // what the cache holds against its limit, as a bar with its legend under it
    let w = ui.available_width();
    let (bar, _) = ui.allocate_exact_size(egui::vec2(w, 8.0), egui::Sense::hover());
    let p = ui.painter();
    p.rect_filled(bar, 4.0, t.text_dim.gamma_multiply(0.35));
    let share = used.map_or(0.0, |b| (b as f64 / (f64::from(mb.max(1)) * 1048576.0)).clamp(0.0, 1.0) as f32);
    if share > 0.0 {
        p.rect_filled(egui::Rect::from_min_size(bar.min, egui::vec2((w * share).max(8.0), bar.height())), 4.0, t.accent);
    }
    register(ui, "bar:settingsCache", bar);
    let size = |b: f64| if b >= 1024.0 * 1048576.0 { format!("{:.2} GB", b / (1024.0 * 1048576.0)) } else { format!("{:.1} MB", b / 1048576.0) };
    let legend = |ui: &mut egui::Ui, dot: egui::Color32, label: &str, value: String| {
        let (r, _) = ui.allocate_exact_size(egui::vec2(w, 28.0), egui::Sense::hover());
        let p = ui.painter();
        p.circle_filled(egui::pos2(r.left() + 7.0, r.center().y), 6.0, dot);
        p.text(egui::pos2(r.left() + 22.0, r.center().y), egui::Align2::LEFT_CENTER, label, t.font(15.0), t.text);
        p.text(egui::pos2(r.right(), r.center().y), egui::Align2::RIGHT_CENTER, value, t.font(15.0), t.text);
    };
    let in_use = used.map_or_else(|| crate::i18n::tr("memory only").to_string(), |b| size(b as f64));
    legend(ui, t.accent, crate::i18n::tr("Cached thumbnails and previews"), in_use);
    legend(ui, t.text_dim.gamma_multiply(0.6), crate::i18n::tr("Size limit"), size(f64::from(mb) * 1048576.0));
    heading(ui, t, crate::i18n::tr("Manage storage"));
    ui.label(RichText::new(crate::i18n::tr("Cache size limit")).color(t.text));
    ui.spacing_mut().slider_width = (ui.available_width() - 90.0).max(120.0);
    let r = ui.add(
        egui::Slider::new(&mut mb, 256..=8192)
            .step_by(256.0)
            .custom_formatter(|v, _| if v >= 1024.0 { format!("{:.1} GB", v / 1024.0) } else { format!("{v:.0} MB") }),
    );
    register(ui, "slider:settingsCache", r.rect);
    if r.dragged() {
        ui.data_mut(|d| d.insert_temp(key, mb));
    } else if r.changed() || r.drag_stopped() {
        ui.data_mut(|d| d.remove::<u32>(key));
        if mb != cur {
            let _ = app.run("library.preferences", json!({"cacheMb": mb}));
        }
    }
    hint(ui, t, crate::i18n::tr("Thumbnails and previews kept on this device so the library opens fast. Older ones go first when it's full."));
    heading(ui, t, crate::i18n::tr("Clear cache"));
    hint(ui, t, crate::i18n::tr("Your photos and edits stay: thumbnails and previews are made again as you browse."));
    ui.vertical_centered(|ui| {
        let r = crate::button::Button::new(crate::i18n::tr("Clear Cache"))
            .enabled(used.is_some_and(|b| b > 0))
            .widget("button:settingsClearCache")
            .min_width(160.0)
            .show(ui);
        if r.clicked() {
            let _ = app.run("library.clearPreviews", json!({}));
        }
    });
    smart_preview_size(app, ui, t);
}

/// How much the smart previews take on the phone or tablet (they stay in the library there).
fn smart_preview_size(app: &mut LightcraftApp, ui: &mut egui::Ui, t: &Tokens) {
    // (listing a big folder every frame would be slow: every 2 s, as the desktop's section does)
    let cache = egui::Id::unique("smart-size");
    let now = ui.input(|i| i.time);
    let loc: Value = match ui.data(|d| d.get_temp::<(f64, Value)>(cache)) {
        Some((at, v)) if now - at < 2.0 => v,
        _ => {
            let v = app.run("library.smartPreviewsLocation", json!({})).unwrap_or(Value::Null);
            ui.data_mut(|d| d.insert_temp(cache, (now, v.clone())));
            v
        }
    };
    if loc.is_null() {
        return;
    }
    heading(ui, t, crate::i18n::tr("Smart previews"));
    let (count, bytes) = (loc["count"].as_u64().unwrap_or(0), loc["bytes"].as_u64().unwrap_or(0));
    let text =
        crate::i18n::tr_format!("{count} smart preview{} · {:.1} MB", if count == 1 { "" } else { "s" }, bytes as f64 / 1048576.0, count = count);
    let r = ui.label(RichText::new(text).color(t.text));
    register(ui, "text:settingsSmartSize", r.rect);
}

/// GPU and its state, the Detail view's preview size, the memory for caches.
fn rendering(app: &mut LightcraftApp, ui: &mut egui::Ui, t: &Tokens) {
    use lightcraft_engine::gpu;
    heading(ui, t, crate::i18n::tr("Rendering"));
    #[cfg(target_os = "windows")]
    check(ui, "settings.composeWindow", &mut app.ui.settings.compose_window, "Prevent G-SYNC/FreeSync flicker (applies at next launch)");
    check(ui, "settings.gpu", &mut app.ui.settings.gpu, "Use the GPU for rendering");
    // `available` would wait for a device still being created (issue #136)
    let status = if gpu::enabled() && !gpu::ready() {
        // the device is created on another thread, which doesn't wake the UI when it's done;
        // make sure that thread runs (cheap and idempotent: a start lost to an off/on switch
        // can't leave "starting" for good)
        gpu::warm_up();
        ui.ctx().request_repaint_after(std::time::Duration::from_millis(250));
        "GPU: starting…".to_string()
    } else if !gpu::available() {
        match gpu::unavailable_reason() {
            Some(why) => crate::i18n::tr_format!("Rendering on the CPU: {why}", why = why),
            None => "No usable GPU found: rendering on the CPU".to_string(),
        }
    } else {
        format!("GPU: {}", gpu::adapter_name().unwrap_or_else(|| "starting…".into()))
    };
    // One line, the whole text on hover: the reason can be a long driver message (no adapter
    // among the backends …), and the GPU state is process-wide, so it may change between frames;
    // a hint that wrapped would move every control below it under the pointer.
    ui.add(egui::Label::new(RichText::new(crate::i18n::tr(&status)).size(11.0).color(t.text_dim)).truncate()).on_hover_text(&status);
    row(ui, t, crate::i18n::tr("Preview size"), |ui| {
        let opts: Vec<(u32, String)> =
            PREVIEW_LIMITS.iter().map(|e| (*e, if *e == 0 { crate::i18n::tr("Automatic").to_string() } else { format!("{e} px") })).collect();
        let opts: Vec<(u32, &str)> = opts.iter().map(|(e, l)| (*e, l.as_str())).collect();
        choices(ui, "settingsPreview", &opts, &mut app.ui.settings.preview_limit);
    });
    hint(
        ui,
        t,
        crate::i18n::tr(
            "Automatic renders the Detail view at the size it is shown, up to the photo's own pixels. Choose a size to cap it: smaller is faster.",
        ),
    );
    row(ui, t, crate::i18n::tr("Memory for caches"), |ui| {
        let auto = crate::i18n::tr_format!("Automatic ({} MB)", lightcraft_engine::memory::default_budget() >> 20);
        let opts = [(0u32, auto.as_str()), (512, "512 MB"), (1024, "1 GB"), (2048, "2 GB"), (4096, "4 GB")];
        choices(ui, "settingsMemory", &opts, &mut app.ui.settings.memory_mb);
    });
}

/// What the thumbnail cache holds now, and Clear Cache.
fn cache_in_use(app: &mut LightcraftApp, ui: &mut egui::Ui, t: &Tokens) {
    let used = app.session.media.rendered.disk().map(|d| d.size());
    row(ui, t, crate::i18n::tr("In use"), |ui| {
        ui.label(
            RichText::new(used.map(|b| format!("{:.1} MB", b as f64 / 1048576.0)).unwrap_or_else(|| crate::i18n::tr("memory only").to_string()))
                .color(t.text),
        );
        let r = ui.button(crate::i18n::tr("Clear Cache"));
        register(ui, "button:settingsClearCache", r.rect);
        if r.clicked() {
            let _ = app.run("library.clearPreviews", json!({}));
        }
    });
}

/// How long photos seen only in Local are kept (only where there is a Local to browse folders in).
fn local_folders(app: &mut LightcraftApp, ui: &mut egui::Ui, t: &Tokens) {
    if crate::sidebar::SidebarSection::Local.available() {
        heading(ui, t, crate::i18n::tr("Local folders"));
        row(ui, t, crate::i18n::tr("Forget unchanged photos"), |ui| {
            let opts = [(0u32, "Never"), (7, "After 7 days"), (30, "After 30 days"), (90, "After 90 days"), (365, "After a year")];
            let mut v = app.session.forget_local_days;
            if choices(ui, "settingsForgetLocal", &opts, &mut v) {
                let _ = app.run("library.preferences", json!({"forgetLocalDays": v}));
            }
        });
        hint(
            ui,
            t,
            crate::i18n::tr(
                "Photos seen in Local but never added or changed leave the catalog when their folder hasn't been browsed for this long (checked when the library opens). Files stay on disk; browsing the folder shows them again.",
            ),
        );
    }
}

/// Where this library keeps its smart previews (the offline-editing proxies, which can be large):
/// the effective folder, what is in it, and choosing another one.
#[cfg(not(target_arch = "wasm32"))]
fn smart_previews(app: &mut LightcraftApp, ui: &mut egui::Ui, t: &Tokens) {
    // listing a big folder every frame would be slow: refresh every 2 s and after a change
    let cache = egui::Id::unique("smart-location");
    let now = ui.input(|i| i.time);
    let loc: Value = match ui.data(|d| d.get_temp::<(f64, Value)>(cache)) {
        Some((at, v)) if now - at < 2.0 => v,
        _ => {
            let v = app.run("library.smartPreviewsLocation", json!({})).unwrap_or(Value::Null);
            ui.data_mut(|d| d.insert_temp(cache, (now, v.clone())));
            v
        }
    };
    if loc.is_null() {
        return;
    }
    heading(ui, t, crate::i18n::tr("Smart previews"));
    let path = loc["path"].as_str().unwrap_or_default().to_string();
    let available = loc["available"].as_bool().unwrap_or(true);
    let custom = loc["custom"].as_bool().unwrap_or(false);
    row(ui, t, crate::i18n::tr("Folder"), |ui| {
        path_label(
            ui,
            "settingsSmartFolder",
            &path,
            egui::TextStyle::Body.resolve(ui.style()),
            if available { t.text } else { t.reject },
            if custom { 0.0 } else { label_width(ui, "(library default)", egui::TextStyle::Body) },
        );
        if !custom {
            let r = ui.label(RichText::new("(library default)").color(t.text));
            register(ui, "text:settingsSmartDefault", r.rect);
        }
    });
    let (count, bytes) = (loc["count"].as_u64().unwrap_or(0), loc["bytes"].as_u64().unwrap_or(0));
    if available {
        hint(
            ui,
            t,
            &crate::i18n::tr_format!(
                "{count} smart preview{} · {:.1} MB",
                if count == 1 { "" } else { "s" },
                bytes as f64 / 1048576.0,
                count = count
            ),
        );
    } else {
        hint(
            ui,
            t,
            crate::i18n::tr(
                "This folder is not available (drive disconnected?). Smart previews are not built or used until it is back or you choose another folder.",
            ),
        );
    }
    // what happens to the previews already built when the folder changes
    let mode_id = egui::Id::unique("smart-existing");
    let mut mode: u8 = ui.data(|d| d.get_temp(mode_id)).unwrap_or(0);
    if count > 0 {
        row(ui, t, crate::i18n::tr("Existing previews"), |ui| {
            if choices(ui, "settingsSmartExisting", &[(0u8, "Move them"), (1, "Leave them"), (2, "Delete them")], &mut mode) {
                ui.data_mut(|d| d.insert_temp(mode_id, mode));
            }
        });
    }
    let existing = ["move", "leave", "discard"][mode.min(2) as usize];
    let mut result = None;
    ui.horizontal(|ui| {
        ui.add_space(LABEL_W);
        if app.services.pick_folder.is_some() {
            let r = ui.button(crate::i18n::tr("Choose Folder…"));
            register(ui, "button:settingsSmartChoose", r.rect);
            if r.clicked()
                && let Some(dir) = app.services.pick_folder.as_mut().and_then(|f| f())
            {
                result = Some(app.run("library.smartPreviewsLocation", json!({"path": dir, "existing": existing})));
            }
        }
        let r = ui.add_enabled(custom, egui::Button::new(crate::i18n::tr("Use Library Folder")));
        register(ui, "button:settingsSmartReset", r.rect);
        if r.clicked() {
            result = Some(app.run("library.smartPreviewsLocation", json!({"reset": true, "existing": existing})));
        }
    });
    if let Some(r) = result {
        ui.data_mut(|d| d.remove::<(f64, Value)>(cache));
        match r {
            Ok(v) => {
                let failed = v["failed"].as_array().map_or(0, Vec::len);
                let msg = match (existing, v["handled"].as_u64().unwrap_or(0)) {
                    (_, 0) => "Smart previews folder changed".to_string(),
                    ("move", n) => format!("Smart previews folder changed; moved {n}"),
                    ("discard", n) => format!("Smart previews folder changed; deleted {n}"),
                    _ => "Smart previews folder changed".to_string(),
                };
                app.toast(ui.ctx(), if failed > 0 { crate::i18n::tr_format!("{msg} ({failed} failed)", failed = failed, msg = msg) } else { msg });
            }
            Err(e) => app.toast(ui.ctx(), e),
        }
    }
    hint(ui, t, crate::i18n::tr("Keep it on a drive with room: smart previews are about 1 MB per photo. The thumbnail cache stays in the library."));
}
