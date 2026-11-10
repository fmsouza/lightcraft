use super::{LABEL_W, RichText, Tokens, register};

/// `path` shortened to fit `max` (as `width` measures it) by dropping leading folders, after
/// either kind of separator, so that its end stays (`…/Pictures/Library`,
/// `…\Pictures\Library`): the end says which folder it is. A last folder too long by itself
/// loses its end ([`crate::widgets::elide_head`]).
pub(super) fn elide_path(path: &str, max: f32, width: impl Fn(&str) -> f32) -> String {
    if width(path) <= max {
        return path.to_string();
    }
    // (a separator at the very end is not where a name starts: `…/` alone would say nothing)
    let end = path.trim_end_matches(['/', '\\']).len();
    let mut rest = path;
    while let Some(i) = rest.find(['/', '\\']).filter(|i| path.len() - rest.len() + i + 1 < end) {
        let separator = if rest.get(i..).is_some_and(|r| r.starts_with('\\')) { '\\' } else { '/' };
        rest = rest.get(i + 1..).unwrap_or("");
        let shorter = format!("…{separator}{rest}");
        if width(&shorter) <= max {
            return shorter;
        }
    }
    let name = rest.trim_end_matches(['/', '\\']);
    // (nothing but separators: never an empty label)
    if name.is_empty() { "…".to_string() } else { crate::widgets::elide_head(name, max, width) }
}

/// A folder or file path on one line in the room left in its row (`text:{id}`): shortened from
/// the front when it is too long ([`elide_path`]), with all of it in a tooltip then. `after` is
/// the width of what follows it on the line (a button, a note) and its spacing: the path leaves
/// that room, so those never wrap onto a second line.
pub(super) fn path_label(ui: &mut egui::Ui, id: &str, path: &str, font: egui::FontId, color: egui::Color32, after: f32) {
    let shown =
        elide_path(path, (ui.available_width() - after).max(0.0), |s| ui.painter().layout_no_wrap(s.to_string(), font.clone(), color).size().x);
    // (`truncate` is the last resort, for a room narrower than "…")
    let r = ui.add(egui::Label::new(RichText::new(&shown).font(font).color(color)).truncate().show_tooltip_when_elided(false));
    register(ui, format!("text:{id}"), r.rect);
    if shown != path {
        r.on_hover_text(path);
    }
}

/// The width of `text` in the style `style`, and the gap before it: what a label takes in a row.
pub(super) fn label_width(ui: &egui::Ui, text: &str, style: egui::TextStyle) -> f32 {
    ui.painter().layout_no_wrap(text.to_string(), style.resolve(ui.style()), egui::Color32::WHITE).size().x + ui.spacing().item_spacing.x
}

/// The same for a button, which has padding either side of its text.
pub(super) fn button_width(ui: &egui::Ui, text: &str) -> f32 {
    label_width(ui, text, egui::TextStyle::Button) + 2.0 * ui.spacing().button_padding.x
}

/// A text box of the Settings window, shown: Esc in it gives its text back and is used up by
/// that, so Settings stays open (a second Esc, with nothing being edited, closes it).
pub(super) fn field(ui: &mut egui::Ui, field: crate::text_field::TextField<'_>) -> crate::text_field::TextFieldResponse {
    let shown = field.show(ui);
    if shown.cancelled() {
        crate::widgets::take_escape(ui.ctx());
    }
    shown
}

pub(crate) fn heading(ui: &mut egui::Ui, t: &Tokens, text: &str) {
    if crate::widgets::finger_ui(ui) {
        // a phone's settings: a line between sections, then the section's name in small capitals
        ui.add_space(6.0);
        ui.separator();
        ui.add_space(4.0);
        ui.label(RichText::new(crate::i18n::tr(text).to_uppercase()).font(t.font(14.0)).color(t.text_dim));
        return;
    }
    ui.add_space(4.0);
    ui.label(RichText::new(crate::i18n::tr(text)).font(t.semibold(12.5)).color(t.text));
}

pub(crate) fn row<R>(ui: &mut egui::Ui, t: &Tokens, label: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    let add = match crate::panels::compact::stacked_row(ui, RichText::new(crate::i18n::tr(label)).color(t.text_label), add) {
        Ok(r) => return r,
        Err(add) => add,
    };
    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(egui::vec2(LABEL_W, 24.0), egui::Layout::left_to_right(egui::Align::Center), |ui| {
            ui.set_min_width(LABEL_W);
            // (a label longer than its column wraps in it; it does not push the controls along)
            ui.add(egui::Label::new(RichText::new(crate::i18n::tr(label)).color(t.text_label)).wrap());
        });
        // The controls wrap in the room beside the label, each further line starting under the
        // first control, so a row with many choices grows down and never widens the window.
        let wrapped = egui::Layout::left_to_right(egui::Align::Center).with_main_wrap(true);
        ui.allocate_ui_with_layout(egui::vec2(ui.available_width(), 24.0), wrapped, |ui| {
            ui.spacing_mut().item_spacing.y = 4.0;
            add(ui)
        })
        .inner
    })
    .inner
}

pub(crate) fn hint(ui: &mut egui::Ui, t: &Tokens, text: &str) {
    let size = if crate::widgets::finger_ui(ui) { 14.0 } else { 11.0 };
    ui.label(RichText::new(crate::i18n::tr(text)).size(size).color(t.text_dim));
}

/// A checkbox addressable as `check:{id}`; true when toggled. Where controls are finger-sized it
/// is a phone's switch: the label on the left, the switch at the right edge, the whole row a target.
pub(crate) fn check(ui: &mut egui::Ui, id: &str, value: &mut bool, label: &str) -> bool {
    if crate::widgets::finger_ui(ui) {
        return switch_row(ui, id, value, crate::i18n::tr(label));
    }
    let r = ui.checkbox(value, crate::i18n::tr(label));
    register(ui, format!("check:{id}"), r.rect);
    r.changed()
}

/// Size of a switch (points).
const SWITCH: egui::Vec2 = egui::vec2(50.0, 30.0);

fn switch_row(ui: &mut egui::Ui, id: &str, value: &mut bool, label: &str) -> bool {
    let t = Tokens::get(ui.ctx());
    let w = ui.available_width();
    let font = t.font(16.0);
    let text_w = (w - SWITCH.x - 16.0).max(40.0);
    let galley = ui.painter().layout(label.to_string(), font, t.text, text_w);
    let h = (galley.size().y + 12.0).max(44.0);
    let (r, mut resp) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::click());
    register(ui, format!("check:{id}"), r);
    if resp.clicked() {
        *value = !*value;
        resp.mark_changed();
    }
    let on = *value;
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, on, label));
    let p = ui.painter();
    p.galley(egui::pos2(r.left(), r.center().y - galley.size().y / 2.0), galley, t.text);
    let how_on = ui.ctx().animate_bool_responsive(resp.id, on);
    let track = egui::Rect::from_center_size(egui::pos2(r.right() - SWITCH.x / 2.0, r.center().y), SWITCH);
    let off_fill = t.text_dim.gamma_multiply(0.35);
    p.rect_filled(track, SWITCH.y / 2.0, crate::widgets::lerp(off_fill, t.accent, how_on));
    let knob_x = egui::lerp((track.left() + SWITCH.y / 2.0)..=(track.right() - SWITCH.y / 2.0), how_on);
    p.circle_filled(egui::pos2(knob_x, track.center().y), SWITCH.y / 2.0 - 3.0, egui::Color32::WHITE);
    resp.changed()
}

/// Mutually exclusive buttons (`button:{id}-{index}`). Upright on a phone, a list with a tick by
/// the one chosen, as a phone's settings have it ([`tick_list`]).
pub(crate) fn choices<V: PartialEq + Copy>(ui: &mut egui::Ui, id: &str, options: &[(V, &str)], value: &mut V) -> bool {
    if phone_list(ui) {
        return tick_list(ui, id, options, value, |_| None);
    }
    let mut changed = false;
    ui.spacing_mut().item_spacing.x = 4.0;
    for (i, (v, l)) in options.iter().enumerate() {
        if crate::widgets::text_button(ui, &format!("{id}-{i}"), l, *value == *v).clicked() && *value != *v {
            *value = *v;
            changed = true;
        }
    }
    changed
}

/// Upright on a phone, with finger-sized controls: choices are lists ([`tick_list`]).
pub(crate) fn phone_list(ui: &egui::Ui) -> bool {
    crate::widgets::finger_ui(ui) && crate::panels::settings::portrait_phone(ui.ctx())
}

/// A phone's list of choices: one row each (`button:{id}-{index}`), the chosen one ticked, lines
/// between; `swatch` may give a row two colours to show before its name (a theme's). True when
/// the choice changed.
pub(crate) fn tick_list<V: PartialEq + Copy>(
    ui: &mut egui::Ui,
    id: &str,
    options: &[(V, &str)],
    value: &mut V,
    swatch: impl Fn(&V) -> Option<(egui::Color32, egui::Color32)>,
) -> bool {
    let t = Tokens::get(ui.ctx());
    let mut changed = false;
    ui.vertical(|ui| {
        let w = ui.available_width();
        ui.set_width(w);
        ui.spacing_mut().item_spacing.y = 0.0;
        for (i, (v, label)) in options.iter().enumerate() {
            let (r, resp) = ui.allocate_exact_size(egui::vec2(w, 44.0), egui::Sense::click());
            register(ui, format!("button:{id}-{i}"), r);
            let label = crate::i18n::tr(label);
            let on = *value == *v;
            resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::RadioButton, true, on, label));
            let p = ui.painter();
            if resp.is_pointer_button_down_on() {
                p.rect_filled(r, 6.0, t.hover);
            }
            let mut x = r.left() + 4.0;
            if let Some((back, front)) = swatch(v) {
                let s = egui::Rect::from_center_size(egui::pos2(x + 13.0, r.center().y), egui::vec2(26.0, 26.0));
                p.rect_filled(s, 6.0, back);
                p.rect_stroke(s, 6.0, egui::Stroke::new(1.0, t.divider), egui::StrokeKind::Inside);
                p.circle_filled(s.center(), 6.0, front);
                x += 36.0;
            }
            p.text(egui::pos2(x, r.center().y), egui::Align2::LEFT_CENTER, label, t.font(16.0), t.text);
            if on {
                let tick = egui::Rect::from_center_size(egui::pos2(r.right() - 14.0, r.center().y), egui::vec2(18.0, 18.0));
                crate::icons::paint(p, tick, crate::icons::Icon::Check, t.accent);
            }
            if i + 1 < options.len() {
                p.hline(x..=r.right(), r.bottom() - 0.5, egui::Stroke::new(1.0, t.divider));
            }
            if resp.clicked() && !on {
                *value = *v;
                changed = true;
            }
        }
    });
    changed
}

#[cfg(test)]
mod path_tests {
    use super::elide_path;

    /// Width = characters, so a limit of 20 is "20 characters".
    fn fit(path: &str, max: usize) -> String {
        elide_path(path, max as f32, |s| s.chars().count() as f32)
    }

    #[test]
    fn a_path_that_fits_is_left_alone() {
        assert_eq!(fit("/Users/me/Pictures", 18), "/Users/me/Pictures");
        assert_eq!(fit("", 5), "");
    }

    #[test]
    fn leading_folders_go_first_and_the_end_stays() {
        assert_eq!(fit("/Users/me/Pictures/Library", 20), "…/Pictures/Library");
        assert_eq!(fit("/Users/me/Pictures/Library", 10), "…/Library");
    }

    /// Windows paths: `Path::display` gives backslashes there.
    #[test]
    fn a_path_with_backslashes_keeps_its_end_too() {
        assert_eq!(fit(r"C:\Users\me\Pictures\Library", 20), r"…\Pictures\Library");
        assert_eq!(fit(r"C:\Users\me/Pictures\Library", 10), r"…\Library");
    }

    #[test]
    fn a_last_folder_too_long_by_itself_loses_its_end() {
        assert_eq!(fit("/Users/me/a-very-long-folder-name", 8), "a-very-…");
        assert_eq!(fit("no-separator-at-all", 8), "no-sepa…");
        assert_eq!(fit("/Users/Фотографии-из-отпуска", 8), "Фотогра…");
    }

    #[test]
    fn a_separator_at_the_end_is_not_where_the_name_starts() {
        assert_eq!(fit("/Users/me/Pictures/Library/", 11), "…/Library/");
        assert_eq!(fit("/Users/me/a-very-long-folder-name/", 8), "a-very-…");
        assert_eq!(fit(r"C:\Users\me\Library\", 11), r"…\Library\");
        assert_eq!(fit("///", 2), "…");
        assert_eq!(fit(r"\\server\share\folder", 15), r"…\share\folder");
    }
}
