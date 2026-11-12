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

/// [`row`], but upright on a phone the control is drawn alone: it names itself there (a
/// [`disclosure`] row carries its label).
pub(crate) fn named_row<R>(ui: &mut egui::Ui, t: &Tokens, label: &str, add: impl FnOnce(&mut egui::Ui) -> R) -> R {
    if phone_list(ui) { add(ui) } else { row(ui, t, label, add) }
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

/// One row of a phone's list (`button:{id}`), ticked when `on`; true when tapped.
pub(crate) fn tick_row(ui: &mut egui::Ui, id: &str, label: &str, on: bool) -> bool {
    let t = Tokens::get(ui.ctx());
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(egui::vec2(w, 44.0), egui::Sense::click());
    register(ui, format!("button:{id}"), r);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::RadioButton, true, on, label));
    let p = ui.painter();
    if resp.is_pointer_button_down_on() {
        p.rect_filled(r, 6.0, t.hover);
    }
    p.text(egui::pos2(r.left() + 4.0, r.center().y), egui::Align2::LEFT_CENTER, label, t.font(16.0), t.text);
    if on {
        let tick = egui::Rect::from_center_size(egui::pos2(r.right() - 14.0, r.center().y), egui::vec2(18.0, 18.0));
        crate::icons::paint(p, tick, crate::icons::Icon::Check, t.accent);
    }
    resp.clicked()
}

/// The page a phone's [`disclosure`] row opened: its choices, drawn in place of the section's
/// page by [`super::window`] (as a phone's settings push a page for a list), until one is picked
/// or ‹ goes back.
#[derive(Clone, Debug)]
pub(crate) struct SubPage {
    pub id: String,
    pub title: String,
    /// (label, is a group's heading)
    pub options: Vec<(String, bool)>,
    pub current: Option<usize>,
}

pub(crate) fn subpage_id() -> egui::Id {
    egui::Id::unique("settings-subpage")
}

fn picked_id() -> egui::Id {
    egui::Id::unique("settings-subpage-picked")
}

/// The open [`SubPage`], if any.
pub(crate) fn subpage(ctx: &egui::Context) -> Option<SubPage> {
    ctx.data(|d| d.get_temp::<SubPage>(subpage_id()))
}

/// Push a page of the caller's own (`id`, shown under `title` with ‹ back): Settings asks
/// [`crate::panels::settings::own_page`] to draw it.
pub(crate) fn open_subpage(ctx: &egui::Context, id: &str, title: &str) {
    ctx.data_mut(|d| d.insert_temp(subpage_id(), SubPage { id: id.to_string(), title: title.to_string(), options: Vec::new(), current: None }));
}

pub(crate) fn close_subpage(ctx: &egui::Context) {
    ctx.data_mut(|d| d.remove::<SubPage>(subpage_id()));
}

/// Draw the open sub-page's choices (one 44 pt row each, the chosen one ticked; headings in
/// small capitals). A pick closes it; the row that opened it returns the pick on the next frame.
pub(crate) fn subpage_list(ui: &mut egui::Ui, page: &SubPage) {
    let t = Tokens::get(ui.ctx());
    for (i, (label, header)) in page.options.iter().enumerate() {
        if *header {
            ui.add_space(6.0);
            ui.label(RichText::new(label.to_uppercase()).font(t.font(13.0)).color(t.text_dim));
            continue;
        }
        if tick_row(ui, &format!("{}List-{i}", page.id), label, page.current == Some(i)) {
            let id = page.id.clone();
            ui.ctx().data_mut(|d| {
                d.insert_temp(picked_id(), (id, i));
                d.remove::<SubPage>(subpage_id());
            });
            ui.ctx().request_repaint();
        }
    }
}

/// A phone's setting with a list of values: a row with its name, the value chosen and › at the
/// right (`combo:{id}`), which opens the choices as a page of their own ([`SubPage`]): `options`
/// are (label, is a heading). The index picked comes back on the frame after.
pub(crate) fn disclosure(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    value: &str,
    options: Vec<(String, bool)>,
    current: Option<usize>,
) -> Option<usize> {
    let t = Tokens::get(ui.ctx());
    let picked = ui.ctx().data_mut(|d| {
        let p = d.get_temp::<(String, usize)>(picked_id()).filter(|(i, _)| i == id);
        if p.is_some() {
            d.remove::<(String, usize)>(picked_id());
        }
        p.map(|(_, i)| i)
    });
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(egui::vec2(w, 44.0), egui::Sense::click());
    register(ui, format!("combo:{id}"), r);
    let label = crate::i18n::tr(label);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, true, format!("{label}: {value}")));
    if resp.clicked() {
        let page = SubPage { id: id.to_string(), title: label.to_string(), options, current };
        ui.ctx().data_mut(|d| d.insert_temp(subpage_id(), page));
    }
    let p = ui.painter();
    if resp.is_pointer_button_down_on() {
        p.rect_filled(r, 6.0, t.hover);
    }
    p.text(egui::pos2(r.left() + 4.0, r.center().y), egui::Align2::LEFT_CENTER, label, t.font(16.0), t.text);
    let chevron = egui::Rect::from_center_size(egui::pos2(r.right() - 10.0, r.center().y), egui::vec2(14.0, 14.0));
    crate::icons::paint(p, chevron, crate::icons::Icon::ChevronRight, t.text_dim);
    // the value, cut short where the name leaves too little room
    let name_w = p.layout_no_wrap(label.to_string(), t.font(16.0), t.text).size().x;
    let room = (w - name_w - 48.0).max(40.0);
    let shown = crate::widgets::one_line(p, value, t.font(16.0), t.text_dim, room);
    let at = egui::pos2(chevron.left() - 8.0 - shown.size().x, r.center().y - shown.size().y / 2.0);
    p.galley(at, shown, t.text_dim);
    picked
}

/// What a [`phone_slider`] did this frame.
pub(crate) struct Slid {
    /// The value moved (while the finger is down too).
    pub changed: bool,
    /// The finger let go after moving it, or tapped the track: the time to apply it.
    pub released: bool,
}

/// A phone's slider, as the system's: its name with the value at the right on one line, then a
/// thin track the width of the page, filled with the accent up to a round white thumb; dragging
/// anywhere on it or tapping it sets the value (`slider:{id}`). `log`: logarithmic (durations);
/// `step`: the value snaps to it.
#[allow(clippy::too_many_arguments)]
pub(crate) fn phone_slider(
    ui: &mut egui::Ui,
    id: &str,
    label: &str,
    value: &mut f64,
    range: std::ops::RangeInclusive<f64>,
    log: bool,
    step: Option<f64>,
    text: impl Fn(f64) -> String,
) -> Slid {
    let t = Tokens::get(ui.ctx());
    let (lo, hi) = (*range.start(), *range.end());
    let w = ui.available_width();
    let (head, _) = ui.allocate_exact_size(egui::vec2(w, 26.0), egui::Sense::hover());
    let label = crate::i18n::tr(label);
    ui.painter().text(egui::pos2(head.left() + 4.0, head.center().y), egui::Align2::LEFT_CENTER, label, t.font(16.0), t.text);
    let (r, resp) = ui.allocate_exact_size(egui::vec2(w, 40.0), egui::Sense::click_and_drag());
    register(ui, format!("slider:{id}"), r);
    const THUMB: f32 = 14.0;
    let track = egui::Rect::from_min_max(egui::pos2(r.left() + THUMB, r.center().y - 2.0), egui::pos2(r.right() - THUMB, r.center().y + 2.0));
    let log = log && lo > 0.0 && hi > lo;
    let to_t = |v: f64| {
        let v = v.clamp(lo, hi);
        if log {
            (v / lo).ln() / (hi / lo).ln()
        } else if hi > lo {
            (v - lo) / (hi - lo)
        } else {
            0.0
        }
    };
    let from_t = |f: f64| {
        let f = f.clamp(0.0, 1.0);
        let v = if log { lo * (hi / lo).powf(f) } else { lo + (hi - lo) * f };
        let v = match step {
            Some(s) if s > 0.0 => lo + ((v - lo) / s).round() * s,
            _ => v,
        };
        v.clamp(lo, hi)
    };
    let before = *value;
    if (resp.dragged() || resp.clicked() || resp.is_pointer_button_down_on())
        && let Some(p) = resp.interact_pointer_pos()
    {
        *value = from_t(f64::from((p.x - track.left()) / track.width().max(1.0)));
    }
    let shown = text(*value);
    resp.widget_info(|| egui::WidgetInfo::slider(true, *value, label));
    ui.painter().text(egui::pos2(head.right() - 4.0, head.center().y), egui::Align2::RIGHT_CENTER, shown, t.font(16.0), t.text_dim);
    let p = ui.painter();
    let x = track.left() + track.width() * to_t(*value) as f32;
    p.rect_filled(track, 2.0, t.text_dim.gamma_multiply(0.35));
    p.rect_filled(egui::Rect::from_min_max(track.min, egui::pos2(x, track.max.y)), 2.0, t.accent);
    let c = egui::pos2(x, track.center().y);
    p.circle_filled(c + egui::vec2(0.0, 1.5), THUMB, egui::Color32::from_black_alpha(50));
    p.circle_filled(c, THUMB, egui::Color32::WHITE);
    p.circle_stroke(c, THUMB, egui::Stroke::new(0.5, egui::Color32::from_black_alpha(40)));
    let changed = *value != before;
    Slid { changed, released: resp.drag_stopped() || (resp.clicked() && !resp.dragged()) }
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

/// How a [`phone_button`] looks: filled with the accent (the screen's main action), tinted (the
/// others), or tinted red (one that removes something).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PhoneButton {
    Filled,
    Tinted,
    Destructive,
}

/// A phone's button, as the system's: the width of the page, 50 pt tall, rounded, its label in
/// semibold 17 pt (`button:{id}`).
pub(crate) fn phone_button(ui: &mut egui::Ui, id: &str, label: &str, style: PhoneButton, enabled: bool) -> bool {
    let t = Tokens::get(ui.ctx());
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(egui::vec2(w, 50.0), if enabled { egui::Sense::click() } else { egui::Sense::hover() });
    register(ui, format!("button:{id}"), r);
    resp.widget_info(|| egui::WidgetInfo::labeled(egui::Role::Button, enabled, label));
    let (fill, text) = match style {
        PhoneButton::Filled => (t.accent, egui::Color32::WHITE),
        PhoneButton::Tinted => (t.accent.gamma_multiply(0.18), t.accent),
        PhoneButton::Destructive => (t.reject.gamma_multiply(0.18), t.reject),
    };
    let fade = |c: egui::Color32| if enabled { c } else { c.gamma_multiply(0.4) };
    let fill = if enabled && resp.is_pointer_button_down_on() { fill.gamma_multiply(0.7) } else { fill };
    let p = ui.painter();
    p.rect_filled(r, 12.0, fade(fill));
    p.text(r.center(), egui::Align2::CENTER_CENTER, label, t.semibold(17.0), fade(text));
    enabled && resp.clicked()
}

/// A tick box with its label, the whole row a target (`check:{id}`): a rounded square, filled
/// with the accent and ticked when on.
pub(crate) fn tick_box(ui: &mut egui::Ui, id: &str, value: &mut bool, label: &str) -> bool {
    let t = Tokens::get(ui.ctx());
    let w = ui.available_width();
    let font = t.font(16.0);
    let galley = ui.painter().layout(label.to_string(), font, t.text, w - 40.0);
    let h = (galley.size().y + 16.0).max(44.0);
    let (r, resp) = ui.allocate_exact_size(egui::vec2(w, h), egui::Sense::click());
    register(ui, format!("check:{id}"), r);
    resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::CheckBox, true, *value, label));
    if resp.clicked() {
        *value = !*value;
    }
    let p = ui.painter();
    let b = egui::Rect::from_center_size(egui::pos2(r.left() + 12.0, r.center().y), egui::vec2(22.0, 22.0));
    if *value {
        p.rect_filled(b, 6.0, t.accent);
        crate::icons::paint(p, b.shrink(3.0), crate::icons::Icon::Check, egui::Color32::WHITE);
    } else {
        p.rect_stroke(b, 6.0, egui::Stroke::new(1.5, t.text_dim), egui::StrokeKind::Inside);
    }
    p.galley(egui::pos2(r.left() + 36.0, r.center().y - galley.size().y / 2.0), galley, t.text);
    resp.clicked()
}
