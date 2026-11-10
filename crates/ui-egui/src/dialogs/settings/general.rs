use super::widgets::{check, choices, field, heading, hint, path_label, row};
use super::{LightcraftApp, RichText, StartupView, Tokens, json, register};

// ------------------------------------------------------------------------------------- General

pub(super) fn general_tab(app: &mut LightcraftApp, ui: &mut egui::Ui, t: &Tokens) {
    let mobile = super::mobile(app);
    let languages: Vec<_> = crate::i18n::Locale::for_picker().into_iter().map(|language| (language, language.name())).collect();
    if super::widgets::phone_list(ui) {
        // upright on a phone: Language and the one chosen, which opens the list
        heading(ui, t, crate::i18n::tr("Language"));
        let shown = languages.iter().find(|(l, _)| *l == app.ui.language).map_or("", |(_, n)| *n);
        let mut pick = app.ui.language;
        let list = |ui: &mut egui::Ui| super::widgets::tick_list(ui, "settingsLanguageList", &languages, &mut pick, |_| None).then_some(());
        if super::widgets::disclosure(ui, "settingsLanguage", "Language", shown, list).is_some() {
            app.ui.language = pick;
        }
    } else {
        row(ui, t, crate::i18n::tr("Language"), |ui| {
            if mobile {
                // a phone held sideways, a tablet: one dropdown, not twenty buttons
                language_dropdown(ui, &languages, &mut app.ui.language);
            } else {
                // Thirteen language names do not fit one line in a narrow window: wrap them.
                ui.horizontal_wrapped(|ui| {
                    choices(ui, "settingsLanguage", &languages, &mut app.ui.language);
                });
            }
        });
    }
    crate::i18n::set_language(app.ui.language);
    // a phone or tablet app keeps one library in its own storage, imports no Lightroom catalogs
    // and opens on its photos: none of these choices apply there
    if !mobile {
        library_rows(app, ui, t);
        heading(ui, t, crate::i18n::tr("Startup"));
        row(ui, t, crate::i18n::tr("Open in"), |ui| {
            choices(
                ui,
                "settingsStartup",
                &[(StartupView::Last, "Last view"), (StartupView::Grid, "Photo Grid"), (StartupView::Detail, "Detail View")],
                &mut app.ui.settings.startup_view,
            );
        });
    }
    heading(ui, t, crate::i18n::tr("Culling"));
    check(ui, "settings.autoAdvance", &mut app.ui.auto_advance, "Auto Advance: move to the next photo after rating or flagging");
    check(ui, "settings.confirmDelete", &mut app.ui.settings.confirm_delete, "Confirm before moving photos to Recently Deleted");
    if mobile {
        // (no folders on disk to move photos to; a sheet closes with a tap outside it)
        return;
    }
    check(ui, "settings.askBeforeMove", &mut app.ui.settings.ask_before_move, "Confirm before moving photos to another folder on disk");
    heading(ui, t, crate::i18n::tr("Dialogs"));
    check(ui, "settings.closeDialogsOutside", &mut app.ui.settings.close_dialogs_outside, "Close a dialog by clicking outside it");
    external_editing(app, ui, t);
}

/// Language as one dropdown (`combo:settingsLanguage`).
fn language_dropdown(ui: &mut egui::Ui, languages: &[(crate::i18n::Locale, &str)], current: &mut crate::i18n::Locale) {
    let shown = languages.iter().find(|(l, _)| l == current).map_or("", |(_, n)| *n);
    let r = crate::widgets::combo_box("settingsLanguage").width(ui.available_width().min(260.0)).selected_text(shown).show_ui(ui, |ui| {
        for (l, name) in languages {
            ui.selectable_value(current, *l, *name);
        }
    });
    crate::widgets::name_combo(&r.response, crate::i18n::tr("Language"));
    register(ui, "combo:settingsLanguage", r.response.rect);
}

/// The library's location, Open / New Library, the one to open at launch, and where a Lightroom
/// catalog goes: a computer's libraries are folders.
fn library_rows(app: &mut LightcraftApp, ui: &mut egui::Ui, t: &Tokens) {
    heading(ui, t, crate::i18n::tr("Library"));
    let location = match &app.session.library {
        Some(l) => l.dir.display().to_string(),
        None => crate::i18n::tr("In memory — nothing is saved").to_string(),
    };
    row(ui, t, crate::i18n::tr("Location"), |ui| {
        path_label(ui, "settingsLocation", &location, egui::TextStyle::Body.resolve(ui.style()), t.text, 0.0);
    });
    row(ui, t, "", |ui| {
        let can = app.services.pick_folder.is_some();
        let r = ui.add_enabled(can, egui::Button::new(crate::i18n::tr("Open Library…")));
        register(ui, "button:settingsOpenLibrary", r.rect);
        if r.clicked() {
            let _ = app.run("app.openLibrary", json!({}));
        }
        let r = ui.add_enabled(can, egui::Button::new(crate::i18n::tr("New Library…")));
        register(ui, "button:settingsNewLibrary", r.rect);
        if r.clicked() {
            let _ = app.run("app.newLibrary", json!({}));
        }
        if !can {
            hint(ui, t, crate::i18n::tr("not available here"));
        }
    });
    // where libraries are folders to choose from (not in the browser, which keeps one)
    if app.services.pick_folder.is_some() {
        use crate::state::LaunchLibrary;
        row(ui, t, crate::i18n::tr("At launch"), |ui| {
            choices(
                ui,
                "settingsAtLaunch",
                &[(LaunchLibrary::Last, "Open the last library"), (LaunchLibrary::Ask, "Ask me")],
                &mut app.ui.settings.at_launch,
            );
        });
        hint(ui, t, crate::i18n::tr("Ask me lists your recent libraries at launch. Holding Alt (Option on a Mac) as the window appears asks once."));
    }
    // File ▸ Import Lightroom Catalog… (#2672): the window that asks where a catalog goes, or not
    ui.label(RichText::new(crate::i18n::tr("When importing a Lightroom catalog")).color(t.text_label));
    row(ui, t, "", |ui| {
        use crate::state::LightroomImport;
        choices(
            ui,
            "settingsLightroomImport",
            &[(LightroomImport::Ask, "Ask where to import it"), (LightroomImport::AlwaysMerge, "Always merge into the open library")],
            &mut app.ui.settings.lightroom_import,
        );
    });
}

/// Edit In: PhotoCraft and another editor, and the file each is handed.
fn external_editing(app: &mut LightcraftApp, ui: &mut egui::Ui, t: &Tokens) {
    heading(ui, t, crate::i18n::tr("External Editing"));
    row(ui, t, crate::i18n::tr("PhotoCraft"), |ui| {
        field(
            ui,
            crate::text_field::TextField::singleline("field:photocraftPath", &mut app.ui.settings.photocraft_path)
                .hint(crate::i18n::tr("Found automatically"))
                .width(220.0),
        );
    });
    edit_file_rows(ui, t, "photocraftFile", &mut app.ui.settings.photocraft_file);
    hint(
        ui,
        t,
        crate::i18n::tr(
            "Photo ▸ Edit In ▸ Edit in PhotoCraft (⇧⌘E, or right-click a photo) renders a copy with your edits, stacks it with the original and opens it in PhotoCraft. Save there and the copy updates here. As PSD, PhotoCraft saves straight back to it. Leave the location empty unless you use a portable copy.",
        ),
    );
    row(ui, t, crate::i18n::tr("Other editor"), |ui| {
        field(
            ui,
            crate::text_field::TextField::singleline("field:externalEditor", &mut app.ui.settings.external_editor)
                .hint(crate::i18n::tr("System default"))
                .width(220.0),
        );
    });
    edit_file_rows(ui, t, "externalFile", &mut app.ui.settings.external_file);
    hint(ui, t, crate::i18n::tr("Edit in Other Editor (⌥⌘E) opens the copy in this application (an app name on macOS, a program path elsewhere)."));
    hint(
        ui,
        t,
        crate::i18n::tr(
            "To hand this editor the original file itself instead (the raw, without a copy: DxO PhotoLab, for example), right-click a photo → Edit In → Open Original in Other Editor (RAW, no copy).",
        ),
    );
}

/// The file Edit In hands an editor: format, colour space and bit depth (`button:{id}Format-…`).
fn edit_file_rows(ui: &mut egui::Ui, t: &Tokens, id: &str, file: &mut crate::state::EditFile) {
    use crate::state::{EditFormat, EditSpace};
    row(ui, t, "", |ui| {
        choices(ui, &format!("{id}Format"), &[(EditFormat::Psd, "PSD"), (EditFormat::Tiff, "TIFF")], &mut file.format);
        ui.separator();
        choices(ui, &format!("{id}Depth"), &[(16u8, "16-bit"), (8, "8-bit")], &mut file.bit_depth);
    });
    row(ui, t, "", |ui| {
        choices(
            ui,
            &format!("{id}Space"),
            &[(EditSpace::AdobeRgb, "Adobe RGB"), (EditSpace::ProPhoto, "ProPhoto"), (EditSpace::DisplayP3, "P3"), (EditSpace::Srgb, "sRGB")],
            &mut file.color_space,
        );
    });
}

// -------------------------------------------------------------------------------------- Import

/// A preset picker: `None` = `none_label`. Returns the new choice when it changed.
pub(super) fn preset_combo(
    app: &LightcraftApp,
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    current: Option<&str>,
    none_label: &str,
) -> Option<Option<String>> {
    let name = |pid: &str| {
        app.session
            .presets
            .iter()
            .find(|p| p.id == pid)
            .map(|p| crate::i18n::builtin_label(&p.name, p.builtin).to_string())
            .unwrap_or_else(|| crate::i18n::tr_format!("{pid} (missing)", pid = pid))
    };
    let text = current.map(name).unwrap_or_else(|| crate::i18n::tr(none_label).to_string());
    if super::widgets::phone_list(ui) {
        // upright on a phone: the row opens the presets under it
        let picked = super::widgets::disclosure(ui, id, label, &text, |ui| {
            if super::widgets::tick_row(ui, &format!("{id}-none"), crate::i18n::tr(none_label), current.is_none()) {
                return Some(None);
            }
            crate::panels::presets::picker_rows(ui, id, &app.session.presets, current).map(Some)
        });
        return picked.filter(|v| v.as_deref() != current);
    }
    let mut out = None;
    let r = crate::widgets::combo_box(id).width(240.0).selected_text(text).show_ui(ui, |ui| {
        if ui.selectable_label(current.is_none(), crate::i18n::tr(none_label)).clicked() {
            out = Some(None);
        }
        if let Some(pid) = crate::panels::presets::picker_rows(ui, id, &app.session.presets, current) {
            out = Some(Some(pid));
        }
    });
    crate::widgets::name_combo(&r.response, label);
    register(ui, format!("combo:{id}"), r.response.rect);
    out.filter(|v| v.as_deref() != current)
}

/// Cameras of the raws in the library plus those with a stored default, sorted.
pub(super) fn cameras(app: &LightcraftApp) -> Vec<String> {
    let mut v: Vec<String> = app
        .session
        .catalog
        .photos()
        .filter(|p| p.kind == lightcraft_catalog::MediaKind::Raw && !p.meta.camera.is_empty())
        .map(|p| p.meta.camera.clone())
        .chain(app.session.import_defaults.cameras.iter().map(|c| c.camera.clone()))
        .collect();
    v.sort_by_key(|c| c.to_lowercase());
    v.dedup_by(|a, b| a.eq_ignore_ascii_case(b));
    v
}
