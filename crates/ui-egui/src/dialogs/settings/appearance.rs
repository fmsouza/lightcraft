use super::widgets::{check, choices, heading, tick_list};
use super::{RichText, Tokens, register};

/// Appearance Mode (`button:settingsAppearance-{0,1,2}`: Auto, Dark, Light) above the light and
/// dark theme cards (`radio:theme-{id}`), each with a preview of the chosen theme. The card in
/// use is outlined. Applied on the next frame ([`LightcraftApp::sync_theme`]).
pub(super) fn appearance(ui: &mut egui::Ui, t: &Tokens, s: &mut crate::state::AppSettings, system: Option<egui::Theme>) {
    use crate::state::{AppearanceMode as M, DarkTheme, LightTheme};
    if super::widgets::phone_list(ui) {
        phone_appearance(ui, t, s, system);
        return;
    }
    heading(ui, t, crate::i18n::tr("Appearance"));
    ui.label(RichText::new(crate::i18n::tr("Appearance Mode")).color(t.text_label));
    ui.horizontal_wrapped(|ui| {
        choices(ui, "settingsAppearance", &[(M::Auto, "Sync with System"), (M::Dark, "Dark Mode"), (M::Light, "Light Mode")], &mut s.appearance_mode);
    });
    ui.add_space(4.0);
    let light_active = s.theme(system) == s.light_theme.kind();
    let available = ui.available_width();
    let mut cards = |ui: &mut egui::Ui, width: f32| {
        ui.spacing_mut().item_spacing.x = 12.0;
        let light: Vec<_> = LightTheme::ALL.iter().map(|l| (*l, l.kind())).collect();
        theme_card(
            ui,
            t,
            ThemeCard { title: "Light Theme", active: light_active, options: &light, width },
            &mut s.light_theme,
            &mut s.favorite_themes,
        );
        let dark: Vec<_> = DarkTheme::ALL.iter().map(|d| (*d, d.kind())).collect();
        theme_card(ui, t, ThemeCard { title: "Dark Theme", active: !light_active, options: &dark, width }, &mut s.dark_theme, &mut s.favorite_themes);
    };
    if available < 440.0 {
        ui.vertical(|ui| cards(ui, available));
    } else {
        ui.horizontal_top(|ui| cards(ui, ((available - 12.0) / 2.0).floor()));
    }
    ui.add_space(2.0);
}

/// What a theme family's card shows.
struct ThemeCard<'a, V> {
    title: &'a str,
    /// the family in use
    active: bool,
    options: &'a [(V, crate::theme::ThemeKind)],
    width: f32,
}

/// One family's card: its title (and "Active" when it is the one shown), a preview of the
/// selected theme and a radio button per theme.
fn theme_card<V: PartialEq + Copy>(ui: &mut egui::Ui, t: &Tokens, card: ThemeCard<V>, value: &mut V, favorites: &mut Vec<String>) {
    let ThemeCard { title, active, options, width } = card;
    egui::Frame::new()
        .fill(t.inset)
        .stroke(egui::Stroke::new(1.0, if active { t.accent } else { t.field_border }))
        .corner_radius(6)
        .inner_margin(8)
        .show(ui, |ui| {
            ui.vertical(|ui| {
                let inner = (width - 18.0).clamp(120.0, 240.0);
                ui.set_width(inner);
                ui.horizontal(|ui| {
                    ui.label(RichText::new(crate::i18n::tr(title)).font(t.semibold(12.0)).color(t.text));
                    if active {
                        ui.label(RichText::new(crate::i18n::tr("In Use")).size(11.0).color(t.accent));
                    }
                });
                let shown = options.iter().find(|(v, _)| v == value).or(options.first()).map(|(_, k)| *k).unwrap_or_default();
                theme_preview(ui, shown, inner);
                for (v, kind) in options {
                    let row = egui::Rect::from_min_size(ui.cursor().min, egui::vec2(ui.available_width(), 24.0));
                    let hovered = ui.ctx().pointer_hover_pos().is_some_and(|p| row.contains(p));
                    ui.horizontal(|ui| {
                        let r = ui.radio_value(value, *v, crate::i18n::tr(kind.label()));
                        register(ui, format!("radio:theme-{}", kind.id()), r.rect);
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let favorite = favorites.iter().any(|id| id == kind.id());
                            let r = ui
                                .add_visible(favorite || hovered, egui::Button::new("").min_size(egui::vec2(24.0, 24.0)).frame(false))
                                .on_hover_text(crate::i18n::tr(if favorite { "Remove from favorites" } else { "Add to favorites" }));
                            r.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::Role::Button,
                                    true,
                                    format!(
                                        "{}: {}",
                                        crate::i18n::tr(if favorite { "Remove from favorites" } else { "Add to favorites" }),
                                        crate::i18n::tr(kind.label())
                                    ),
                                )
                            });
                            register(ui, format!("button:themeFavorite-{}", kind.id()), r.rect);
                            if favorite || hovered {
                                crate::rating::paint_star(ui.painter(), r.rect.shrink(4.0), favorite, if favorite { t.accent } else { t.text_dim });
                            }
                            if r.clicked() {
                                if favorite {
                                    favorites.retain(|id| id != kind.id());
                                } else {
                                    favorites.push(kind.id().into());
                                }
                            }
                        });
                    });
                }
            });
        });
}

/// A small LightCraft window painted with `kind`'s tokens: the top bar with its search field, the
/// photo with the filmstrip under it, the Edit panel (histogram and sliders), the tool strip and
/// the bottom bar.
fn theme_preview(ui: &mut egui::Ui, kind: crate::theme::ThemeKind, width: f32) {
    use egui::{Color32, Rect, Stroke, StrokeKind, pos2, vec2};
    let p = Tokens::for_kind(kind);
    let (rect, _) = ui.allocate_exact_size(vec2(width, (width * 0.56).round().min(130.0)), egui::Sense::hover());
    let painter = ui.painter_at(rect);
    let bar = |y: f32, x: f32, w: f32, c: Color32| painter.rect_filled(Rect::from_min_size(pos2(x, y), vec2(w, 2.0)), 1.0, c);
    painter.rect_filled(rect, 3.0, p.canvas);
    // top bar: sidebar and back icons, the centred search field, icons on the right
    let top = Rect::from_min_size(rect.min, vec2(rect.width(), 13.0));
    painter.rect_filled(top, 0.0, p.chrome);
    for x in [6.0, 14.0, 20.0] {
        painter.rect_filled(Rect::from_min_size(top.min + vec2(x, 4.5), vec2(4.0, 4.0)), 1.0, p.icon);
    }
    let search = Rect::from_center_size(top.center(), vec2(rect.width() * 0.34, 8.0));
    painter.rect(search, 2.0, p.field, Stroke::new(1.0, p.field_border), StrokeKind::Inside);
    bar(search.center().y - 1.0, search.center().x - 9.0, 18.0, p.text_dim);
    for i in 0..4 {
        painter.rect_filled(Rect::from_min_size(pos2(top.right() - 10.0 - i as f32 * 9.0, top.top() + 4.5), vec2(4.0, 4.0)), 2.0, p.icon);
    }
    // bottom bar: view buttons, rating stars, a button
    let bottom = Rect::from_min_max(pos2(rect.left(), rect.bottom() - 12.0), rect.max);
    painter.rect_filled(bottom, 0.0, p.chrome);
    for x in [6.0, 13.0, 20.0] {
        painter.rect_filled(Rect::from_min_size(bottom.min + vec2(x, 4.0), vec2(4.0, 4.0)), 1.0, p.icon);
    }
    let stars_x = bottom.center().x - 30.0;
    for i in 0..5 {
        painter.circle_filled(pos2(stars_x + i as f32 * 5.0, bottom.center().y), 1.4, p.star);
    }
    let copy = Rect::from_min_size(pos2(stars_x + 28.0, bottom.top() + 2.5), vec2(30.0, 7.0));
    painter.rect(copy, 3.0, p.button, Stroke::new(1.0, p.button_border), StrokeKind::Inside);
    // tool strip on the right edge, the first tool (Edit) active
    let strip = Rect::from_min_max(pos2(rect.right() - 12.0, top.bottom()), pos2(rect.right(), bottom.top()));
    painter.rect_filled(strip, 0.0, p.chrome);
    painter.line_segment([strip.left_top(), strip.left_bottom()], Stroke::new(1.0, p.divider));
    for i in 0..5 {
        let r = Rect::from_min_size(pos2(strip.left() + 3.0, strip.top() + 4.0 + i as f32 * 9.0), vec2(6.0, 6.0));
        if i == 0 {
            painter.rect_filled(r.expand(1.5), 1.5, p.tool_active);
        }
        painter.rect_stroke(r, 1.0, Stroke::new(1.0, if i == 0 { p.text } else { p.icon }), StrokeKind::Inside);
    }
    // Edit panel: histogram, then labelled sliders
    let panel = Rect::from_min_max(pos2(strip.left() - (rect.width() * 0.27).round(), top.bottom()), pos2(strip.left(), bottom.top()));
    painter.rect_filled(panel, 0.0, p.chrome);
    painter.line_segment([panel.left_top(), panel.left_bottom()], Stroke::new(1.0, p.divider));
    let hist = Rect::from_min_size(panel.min + vec2(4.0, 4.0), vec2(panel.width() - 8.0, 14.0));
    painter.rect_filled(hist, 1.0, p.inset);
    let curve = [0.2, 0.55, 0.9, 0.7, 0.5, 0.62, 0.4, 0.3, 0.18, 0.1];
    let mut pts = vec![hist.left_bottom()];
    for (i, h) in curve.iter().enumerate() {
        pts.push(pos2(hist.left() + hist.width() * i as f32 / (curve.len() - 1) as f32, hist.bottom() - hist.height() * h));
    }
    pts.push(hist.right_bottom());
    // the area under the curve, a column per stretch between two points
    for pair in pts.windows(2) {
        if let [a, b] = pair {
            let r = Rect::from_min_max(pos2(a.x, a.y.max(b.y)), pos2(b.x, hist.bottom()));
            painter.rect_filled(r, 0.0, p.track.gamma_multiply(0.5));
        }
    }
    painter.add(egui::Shape::line(pts, Stroke::new(1.0, p.text_dim)));
    let sliders = [0.55, 0.62, 0.35, 0.7, 0.6, 0.45];
    let mut y = hist.bottom() + 7.0;
    bar(y, panel.left() + 5.0, 14.0, p.text);
    y += 6.0;
    for v in sliders {
        if y + 7.0 > panel.bottom() {
            break;
        }
        bar(y, panel.left() + 5.0, 12.0, p.text_label);
        bar(y, panel.right() - 10.0, 5.0, p.text_dim);
        let track_y = y + 5.0;
        painter.line_segment([pos2(panel.left() + 5.0, track_y), pos2(panel.right() - 5.0, track_y)], Stroke::new(1.0, p.track));
        painter.circle_filled(pos2(panel.left() + 5.0 + (panel.width() - 10.0) * v, track_y), 1.8, p.thumb);
        y += 10.0;
    }
    // the photo (its own colours: a photo looks the same in every theme) above the filmstrip
    let film = Rect::from_min_max(pos2(rect.left(), bottom.top() - 18.0), pos2(panel.left(), bottom.top()));
    let stage = Rect::from_min_max(pos2(rect.left(), top.bottom()), pos2(panel.left(), film.top()));
    let photo = Rect::from_center_size(stage.center(), vec2(stage.width() * 0.62, stage.height() - 8.0));
    painter.rect_filled(photo, 0.0, Color32::from_rgb(0x6f, 0x8f, 0xb4));
    let horizon = photo.top() + photo.height() * 0.62;
    painter.add(egui::Shape::convex_polygon(
        vec![
            pos2(photo.left(), horizon),
            pos2(photo.left() + photo.width() * 0.3, photo.top() + photo.height() * 0.3),
            pos2(photo.left() + photo.width() * 0.55, horizon),
        ],
        Color32::from_rgb(0x4a, 0x55, 0x66),
        Stroke::NONE,
    ));
    painter.add(egui::Shape::convex_polygon(
        vec![
            pos2(photo.left() + photo.width() * 0.35, horizon),
            pos2(photo.left() + photo.width() * 0.68, photo.top() + photo.height() * 0.2),
            pos2(photo.right(), horizon),
        ],
        Color32::from_rgb(0x5a, 0x63, 0x72),
        Stroke::NONE,
    ));
    painter.rect_filled(Rect::from_min_max(pos2(photo.left(), horizon), photo.max), 0.0, Color32::from_rgb(0x3d, 0x5a, 0x3a));
    painter.line_segment([film.left_top(), film.right_top()], Stroke::new(1.0, p.divider));
    for i in 0..5 {
        let cell = Rect::from_min_size(pos2(film.left() + 4.0 + i as f32 * 17.0, film.top() + 3.0), vec2(15.0, 12.0));
        if cell.right() > film.right() - 2.0 {
            break;
        }
        let selected = i == 1;
        painter.rect_filled(cell, 0.0, if selected { p.cell_selected } else { p.cell });
        let thumb = cell.shrink(2.5);
        painter.rect_filled(thumb, 0.0, Color32::from_rgb(0x6f + 8 * i as u8, 0x80, 0x8f));
        if selected {
            crate::panels::grid::selection_ring(&painter, &p, thumb, lightcraft_engine::SelectionState::Active);
        }
    }
}

/// Appearance upright on a phone, as a phone's own display settings have it: Light and Dark side
/// by side, each a phone showing the library in its theme with a tick under the one in use, then
/// Automatic (follow the system), then each family's themes as a list.
fn phone_appearance(ui: &mut egui::Ui, t: &Tokens, s: &mut crate::state::AppSettings, system: Option<egui::Theme>) {
    use crate::state::{AppearanceMode as M, DarkTheme, LightTheme};
    heading(ui, t, crate::i18n::tr("Appearance"));
    let light_active = s.theme(system) == s.light_theme.kind();
    let w = ui.available_width();
    let (row, _) = ui.allocate_exact_size(egui::vec2(w, 196.0), egui::Sense::hover());
    for (i, (label, kind, light)) in [("Light", s.light_theme.kind(), true), ("Dark", s.dark_theme.kind(), false)].into_iter().enumerate() {
        let x = row.left() + w * (0.25 + 0.5 * i as f32);
        let cell = egui::Rect::from_center_size(egui::pos2(x, row.center().y), egui::vec2(w * 0.5, row.height()));
        let resp = ui.interact(cell, egui::Id::unique(("appearance-phone", light)), egui::Sense::click());
        let label = crate::i18n::tr(label);
        let on = light_active == light;
        resp.widget_info(|| egui::WidgetInfo::selected(egui::Role::RadioButton, true, on, label));
        register(ui, if light { "button:appearanceLight" } else { "button:appearanceDark" }, cell);
        let phone = egui::Rect::from_min_size(egui::pos2(x - 36.0, cell.top() + 4.0), egui::vec2(72.0, 140.0));
        phone_preview(ui.painter(), phone, kind);
        let p = ui.painter();
        p.text(egui::pos2(x, phone.bottom() + 14.0), egui::Align2::CENTER_CENTER, label, t.font(15.0), t.text);
        let dot = egui::pos2(x, phone.bottom() + 38.0);
        if on {
            p.circle_filled(dot, 11.0, t.accent);
            crate::icons::paint(p, egui::Rect::from_center_size(dot, egui::vec2(14.0, 14.0)), crate::icons::Icon::Check, egui::Color32::WHITE);
        } else {
            p.circle_stroke(dot, 10.5, egui::Stroke::new(1.5, t.text_dim));
        }
        if resp.clicked() {
            s.appearance_mode = if light { M::Light } else { M::Dark };
        }
    }
    let mut auto = s.appearance_mode == M::Auto;
    if check(ui, "settings.appearanceAuto", &mut auto, "Automatic") {
        s.appearance_mode = if auto {
            M::Auto
        } else if light_active {
            M::Light
        } else {
            M::Dark
        };
    }
    super::widgets::hint(ui, t, crate::i18n::tr("Light or dark as the system is set."));
    let swatch = |k: crate::theme::ThemeKind| {
        let p = Tokens::for_kind(k);
        Some((p.chrome, p.accent))
    };
    heading(ui, t, crate::i18n::tr("Light Theme"));
    let light: Vec<_> = LightTheme::ALL.iter().map(|l| (*l, l.kind().label())).collect();
    tick_list(ui, "themeLight", &light, &mut s.light_theme, |l| swatch(l.kind()));
    heading(ui, t, crate::i18n::tr("Dark Theme"));
    let dark: Vec<_> = DarkTheme::ALL.iter().map(|d| (*d, d.kind().label())).collect();
    tick_list(ui, "themeDark", &dark, &mut s.dark_theme, |d| swatch(d.kind()));
}

/// A phone showing the library in `kind`'s colours: the status bar, the album row, a month
/// heading and a grid of photos (their own colours: a photo looks the same in every theme).
fn phone_preview(painter: &egui::Painter, rect: egui::Rect, kind: crate::theme::ThemeKind) {
    use egui::{Color32, Rect, Stroke, StrokeKind, pos2, vec2};
    let p = Tokens::for_kind(kind);
    painter.rect_filled(rect, 12.0, p.canvas);
    painter.rect_stroke(rect, 12.0, Stroke::new(2.0, p.divider), StrokeKind::Outside);
    let inner = rect.shrink(4.0);
    // the island, then the bar with the album name and its buttons
    painter.rect_filled(Rect::from_center_size(pos2(inner.center().x, inner.top() + 6.0), vec2(18.0, 5.0)), 2.5, Color32::BLACK);
    let bar = Rect::from_min_size(pos2(inner.left(), inner.top() + 12.0), vec2(inner.width(), 14.0));
    painter.rect_filled(bar, 0.0, p.chrome);
    painter.rect_filled(Rect::from_min_size(bar.min + vec2(4.0, 5.0), vec2(22.0, 4.0)), 1.0, p.text);
    for i in 0..3 {
        painter.circle_filled(pos2(bar.right() - 5.0 - i as f32 * 8.0, bar.center().y), 2.0, p.icon);
    }
    // a month heading over three columns of photos
    painter.rect_filled(Rect::from_min_size(pos2(inner.left() + 3.0, bar.bottom() + 5.0), vec2(20.0, 3.0)), 1.0, p.text_dim);
    let photos = [(0x6f, 0x8f, 0xb4), (0xb4, 0x8a, 0x5c), (0x5e, 0x7d, 0x5a), (0x8e, 0x6a, 0x8f), (0xc2, 0xa0, 0x6e), (0x4f, 0x63, 0x7a)];
    let cell = (inner.width() - 2.0) / 3.0;
    let top = bar.bottom() + 11.0;
    let mut n = 0;
    'grid: for row in 0.. {
        for col in 0..3 {
            let r = Rect::from_min_size(pos2(inner.left() + col as f32 * (cell + 1.0), top + row as f32 * (cell + 1.0)), vec2(cell, cell));
            if r.bottom() > inner.bottom() - 2.0 {
                break 'grid;
            }
            let (red, green, blue) = photos[n % photos.len()];
            painter.rect_filled(r, 0.0, Color32::from_rgb(red, green, blue));
            n += 1;
        }
    }
}
