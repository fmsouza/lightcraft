//! The phone layout's Share sheet, after the share sheet of phone photo editors: the photos to
//! share (each with a check box), then **Share** (a full-size JPEG, straight to the system share
//! sheet), **Export as** (a short form: file type, size, quality, watermark; More options opens the
//! full Export dialog) and **Save to Photos** (straight into the photo library; its gear sets what
//! is saved) and **Add border** (a plain border round the photo, even or shaping the canvas to
//! 1:1, 4:5 …, then Share). Opened by the Export buttons of [`super::compact`]'s bars.
//!
//! Every action is a background `app.export` of the chosen photos; [`Handoff`] says where its
//! files go once written. The sheet's own exports leave Export with Previous (and the full dialog's
//! settings) as they were.

use egui::{Rect, Sense, Stroke, StrokeKind, vec2};
use lightcraft_catalog::PhotoId;
use serde_json::{Value, json};

use crate::LightcraftApp;
use crate::export_task::Handoff;
use crate::icons::{Icon, paint};
use crate::theme::Tokens;
use crate::widgets::{icon_button, register};

/// File types the form offers: the format's key and its label.
const FORMATS: &[(&str, &str)] =
    &[("original", "Original"), ("jpeg", "JPG"), ("avif", "AVIF"), ("jxl", "JXL"), ("tiff", "TIF"), ("png", "PNG"), ("dng", "DNG")];

/// Long-edge sizes (0: the photo's own).
const SIZES: &[(u32, &str)] = &[(0, "Largest available"), (4096, "4096 px"), (2048, "2048 px"), (1080, "1080 px")];

const QUALITIES: &[u8] = &[100, 92, 85, 70];

/// What one form exports.
#[derive(Clone, Debug, PartialEq)]
pub struct Form {
    pub format: &'static str,
    pub long_edge: u32,
    pub quality: u8,
    pub watermark: bool,
}

impl Default for Form {
    /// What Share sends: a full-size JPEG, the quality of the JPEG (Large) preset.
    fn default() -> Self {
        Self { format: "jpeg", long_edge: 0, quality: 92, watermark: false }
    }
}

impl Form {
    /// A rendered file (not the original or its raw data as DNG): size, quality and watermark apply.
    fn rendered(&self) -> bool {
        !matches!(self.format, "original" | "dng")
    }
    fn lossy(&self) -> bool {
        matches!(self.format, "jpeg" | "avif" | "jxl")
    }
    /// `app.export`'s params for `ids`; `watermark` is the one set in the full Export dialog.
    pub fn params(&self, ids: &[PhotoId], watermark: Option<&Value>) -> Value {
        let ids: Vec<u64> = ids.iter().map(|id| id.0).collect();
        let mut p = json!({"format": self.format, "ids": ids, "background": true});
        if self.rendered() {
            p["longEdge"] = json!(self.long_edge);
            p["dontEnlarge"] = json!(true);
            p["colorSpace"] = json!("srgb");
            if self.lossy() {
                p["quality"] = json!(self.quality);
            }
            if self.watermark
                && let Some(w) = watermark
            {
                p["watermark"] = w.clone();
            }
        }
        p
    }
}

/// Border colours: white, greys, black, and two warm tones.
const BORDER_COLORS: &[[u8; 3]] =
    &[[255, 255, 255], [214, 214, 214], [150, 150, 150], [90, 90, 90], [0, 0, 0], [24, 24, 24], [62, 48, 42], [238, 228, 210]];

/// Border shapes: none, even on every side, or the canvas's width : height.
const BORDER_SHAPES: &[(&str, Option<[u16; 2]>)] = &[
    ("Original", None),
    ("Even", None),
    ("1:1", Some([1, 1])),
    ("4:5", Some([4, 5])),
    ("3:4", Some([3, 4])),
    ("2:3", Some([2, 3])),
    ("3:2", Some([3, 2])),
    ("9:16", Some([9, 16])),
    ("16:9", Some([16, 9])),
];

/// Thickness slider steps: 0..=25, a step being 1 % of the photo's long edge on each side.
const BORDER_STEPS: u8 = 25;

/// What Add border draws.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BorderForm {
    pub color: [u8; 3],
    /// 0..=[`BORDER_STEPS`]: per cent of the photo's long edge.
    pub thickness: u8,
    /// An index into [`BORDER_SHAPES`] (0: no border).
    pub shape: usize,
}

impl Default for BorderForm {
    fn default() -> Self {
        Self { color: [255, 255, 255], thickness: 5, shape: 1 }
    }
}

impl BorderForm {
    /// The export's border, or none for Original.
    pub fn border(&self) -> Option<lightcraft_engine::export::Border> {
        let (_, ratio) = BORDER_SHAPES.get(self.shape).copied().filter(|_| self.shape > 0)?;
        Some(lightcraft_engine::export::Border { color: self.color, thickness: f32::from(self.thickness.min(BORDER_STEPS)) / 100.0, ratio })
    }

    /// `params` with this border added (unchanged for Original).
    pub fn apply(&self, mut params: Value) -> Value {
        if let Some(b) = self.border() {
            // (in per cent from the slider's whole steps: 0.05, not f32's 0.0500000007)
            let thickness = f64::from(self.thickness.min(BORDER_STEPS)) / 100.0;
            params["border"] = json!({"color": b.color, "thickness": thickness, "ratio": b.ratio});
        }
        params
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
enum Page {
    #[default]
    Main,
    /// The form for Export as (`Handoff::Share`) or for Save to Photos.
    Form(Handoff),
    Border,
}

#[derive(Clone, Debug, Default)]
struct Sheet {
    ids: Vec<PhotoId>,
    chosen: Vec<bool>,
    page: Page,
}

/// The forms, kept for the session (Export as, Save to Photos).
#[derive(Clone, Debug, Default)]
struct Forms {
    export_as: Form,
    photos: Form,
    border: BorderForm,
}

fn sheet_id() -> egui::Id {
    egui::Id::unique("compact-share-sheet")
}

fn forms_id() -> egui::Id {
    egui::Id::unique("compact-share-forms")
}

/// The photos the Export buttons share: the editor's photo; in the library the selection (in grid
/// order), else the active photo.
pub fn photos_to_share(app: &mut LightcraftApp, editor: bool) -> Vec<PhotoId> {
    let active = app.session.active();
    if editor || app.session.selection.ids.is_empty() {
        return active.into_iter().collect();
    }
    let sel: std::collections::HashSet<_> = app.session.selection.ids.iter().copied().collect();
    let mut v: Vec<_> = app.session.visible().iter().copied().filter(|id| sel.contains(id)).collect();
    let shown: std::collections::HashSet<_> = v.iter().copied().collect();
    v.extend(app.session.selection.ids.iter().filter(|id| !shown.contains(id)));
    v
}

/// Open the sheet on `ids` (nothing to share: nothing happens).
pub fn open(ctx: &egui::Context, ids: Vec<PhotoId>) {
    if ids.is_empty() {
        return;
    }
    let chosen = vec![true; ids.len()];
    ctx.data_mut(|d| d.insert_temp(sheet_id(), Sheet { ids, chosen, page: Page::Main }));
}

pub fn is_open(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<Sheet>(sheet_id()).is_some())
}

pub(crate) fn close(ctx: &egui::Context) {
    ctx.data_mut(|d| d.remove_temp::<Sheet>(sheet_id()));
}

/// Draw the sheet when it's open: at the bottom of the screen over a dimmed backdrop.
pub fn show(app: &mut LightcraftApp, ctx: &egui::Context) {
    let Some(mut sheet) = ctx.data(|d| d.get_temp::<Sheet>(sheet_id())) else { return };
    let mut forms = ctx.data(|d| d.get_temp::<Forms>(forms_id())).unwrap_or_default();
    let t = Tokens::get(ctx);
    let screen = ctx.content_rect();
    let width = screen.width().min(560.0);
    let area = egui::Area::new(egui::Id::unique("compact-share-area"))
        .anchor(egui::Align2::CENTER_BOTTOM, vec2(0.0, 0.0))
        .default_width(width)
        .order(egui::Order::Foreground);
    let frame = egui::Frame::NONE
        .fill(t.chrome)
        .stroke(Stroke::new(1.0, t.divider))
        .corner_radius(egui::CornerRadius { nw: 14, ne: 14, sw: 0, se: 0 })
        .inner_margin(egui::Margin { left: 16, right: 16, top: 6, bottom: 16 });
    let modal = egui::Modal::new(egui::Id::unique("compact-share-modal")).area(area).frame(frame).show(ctx, |ui| {
        ui.set_width(width - 32.0);
        register(ui, "panel:shareSheet", ui.max_rect());
        match sheet.page {
            Page::Main => main_page(app, ui, &mut sheet, &forms),
            Page::Form(to) => {
                let form = if to == Handoff::Photos { &mut forms.photos } else { &mut forms.export_as };
                form_page(app, ui, &mut sheet, form, to);
            }
            Page::Border => border_page(app, ui, &mut sheet, &mut forms.border),
        }
    });
    ctx.data_mut(|d| d.insert_temp(forms_id(), forms));
    if modal.should_close() {
        if sheet.page == Page::Main {
            close(ctx);
            return;
        }
        sheet.page = Page::Main;
    }
    if is_open(ctx) {
        ctx.data_mut(|d| d.insert_temp(sheet_id(), sheet));
    }
}

/// The sheet's head: ✕ or ‹ at the left, the title, and `right` at the right.
fn head(ui: &mut egui::Ui, back: bool, title: &str, right: impl FnOnce(&mut egui::Ui)) -> bool {
    let t = Tokens::get(ui.ctx());
    let mut left = false;
    ui.horizontal(|ui| {
        ui.set_height(44.0);
        let (icon, tip) = if back { (Icon::ChevronLeft, "Back") } else { (Icon::Close, "Close") };
        left = icon_button(ui, "shareClose", icon, vec2(40.0, 40.0), false, true, tip).clicked();
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            right(ui);
            ui.centered_and_justified(|ui| ui.label(egui::RichText::new(title).font(t.semibold(16.0)).color(t.text)));
        });
    });
    ui.add_space(4.0);
    left
}

fn main_page(app: &mut LightcraftApp, ui: &mut egui::Ui, sheet: &mut Sheet, forms: &Forms) {
    let t = Tokens::get(ui.ctx());
    let n = sheet.ids.len();
    let all = sheet.chosen.iter().all(|c| *c);
    let title = if n == 1 { crate::i18n::tr("Share photo").to_string() } else { crate::i18n::tr_format!("Share {n} photos", n = n) };
    let mut toggle_all = false;
    let closed = head(ui, false, &title, |ui| {
        let label = if all { crate::i18n::tr("Select none") } else { crate::i18n::tr("Select all") };
        let b = ui.add(egui::Button::new(egui::RichText::new(label).font(t.font(14.0)).color(t.accent)).frame(false));
        register(ui, "button:shareSelectAll", b.rect);
        toggle_all = b.clicked();
    });
    if closed {
        close(ui.ctx());
        return;
    }
    if toggle_all {
        sheet.chosen.iter_mut().for_each(|c| *c = !all);
    }
    photos(app, ui, sheet);
    let ids: Vec<PhotoId> = sheet.ids.iter().zip(&sheet.chosen).filter(|(_, c)| **c).map(|(id, _)| *id).collect();
    let any = !ids.is_empty();
    ui.add_space(8.0);
    ui.separator();
    ui.add_space(8.0);
    ui.columns(2, |cols| {
        if round_button(&mut cols[0], "shareNow", Icon::Share, "Share", any) {
            run(app, cols[0].ctx(), &Form::default(), &ids, Handoff::Share);
        }
        if round_button(&mut cols[1], "shareExportAs", Icon::Dots, "Export as", any) {
            sheet.page = Page::Form(Handoff::Share);
        }
    });
    ui.add_space(8.0);
    ui.separator();
    if border_row(ui, any) {
        sheet.page = Page::Border;
    }
    if app.services.save_to_photos.is_some() {
        ui.add_space(8.0);
        ui.separator();
        let (save, gear) = list_row(ui, "shareSavePhotos", Icon::Photos, "Save to Photos", any);
        if save {
            run(app, ui.ctx(), &forms.photos, &ids, Handoff::Photos);
        } else if gear {
            sheet.page = Page::Form(Handoff::Photos);
        }
    }
}

/// The photos: one large with its check box below, or a row of tiles with a check box on each.
fn photos(app: &mut LightcraftApp, ui: &mut egui::Ui, sheet: &mut Sheet) {
    let t = Tokens::get(ui.ctx());
    let ppp = ui.ctx().pixels_per_point();
    if sheet.ids.len() == 1 {
        let avail = ui.available_width();
        let size = vec2(avail, (avail * 0.66).min(ui.ctx().content_rect().height() * 0.35));
        let (r, resp) = ui.allocate_exact_size(size, Sense::click());
        tile(app, ui, sheet.ids[0], r, sheet.chosen[0], (size.max_elem() * ppp) as usize);
        let (cr, cresp) = ui.allocate_exact_size(vec2(avail, 36.0), Sense::click());
        check_box(ui, Rect::from_center_size(cr.center(), vec2(24.0, 24.0)), sheet.chosen[0], &t);
        register(ui, "photo:share:0", r);
        if resp.clicked() || cresp.clicked() {
            sheet.chosen[0] = !sheet.chosen[0];
        }
        return;
    }
    let edge = 96.0;
    egui::ScrollArea::horizontal().id_salt("share-photos").show(ui, |ui| {
        ui.horizontal(|ui| {
            for (i, id) in sheet.ids.clone().into_iter().enumerate() {
                let (r, resp) = ui.allocate_exact_size(vec2(edge, edge), Sense::click());
                if !ui.is_rect_visible(r) {
                    continue;
                }
                tile(app, ui, id, r, sheet.chosen[i], (edge * ppp) as usize);
                check_box(ui, Rect::from_min_size(r.right_bottom() - vec2(28.0, 28.0), vec2(22.0, 22.0)), sheet.chosen[i], &t);
                register(ui, format!("photo:share:{i}"), r);
                if resp.clicked() {
                    sheet.chosen[i] = !sheet.chosen[i];
                }
            }
        });
    });
}

/// A photo's thumbnail fitted into `r`, outlined in the accent colour when chosen.
fn tile(app: &mut LightcraftApp, ui: &egui::Ui, id: PhotoId, r: Rect, chosen: bool, px: usize) {
    let t = Tokens::get(ui.ctx());
    super::grid::request_thumb(app, id, px.max(64), 10);
    let Some(tex) = app.renderer.thumb(id) else {
        ui.painter().rect_filled(r, 4.0, t.divider);
        return;
    };
    let [w, h] = tex.size;
    let scale = (r.width() / w.max(1) as f32).min(r.height() / h.max(1) as f32);
    let shown = Rect::from_center_size(r.center(), vec2(w as f32 * scale, h as f32 * scale));
    let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    ui.painter().image(tex.tex.id(), shown, uv, egui::Color32::WHITE);
    if chosen {
        ui.painter().rect_stroke(shown.expand(1.5), 4.0, Stroke::new(3.0, t.accent), StrokeKind::Outside);
    } else {
        ui.painter().rect_filled(shown, 0.0, egui::Color32::from_black_alpha(110));
    }
}

fn check_box(ui: &egui::Ui, r: Rect, on: bool, t: &Tokens) {
    let p = ui.painter();
    if on {
        p.rect_filled(r, 4.0, t.accent);
        paint(p, r.shrink(3.0), Icon::Check, egui::Color32::WHITE);
    } else {
        p.rect_stroke(r, 4.0, Stroke::new(1.5, t.text), StrokeKind::Inside);
    }
}

/// A round button with its label underneath (Share, Export as); true when tapped.
fn round_button(ui: &mut egui::Ui, id: &str, icon: Icon, label: &str, enabled: bool) -> bool {
    let t = Tokens::get(ui.ctx());
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(vec2(w, 86.0), if enabled { Sense::click() } else { Sense::hover() });
    register(ui, format!("button:{id}"), r);
    let c = egui::pos2(r.center().x, r.top() + 30.0);
    let p = ui.painter();
    let fill = if resp.is_pointer_button_down_on() { t.divider } else { t.chrome };
    p.circle(c, 28.0, fill, Stroke::new(1.0, t.divider));
    let color = if enabled { t.accent } else { t.text_dim };
    paint(p, Rect::from_center_size(c, vec2(24.0, 24.0)), icon, color);
    let text = if enabled { t.text } else { t.text_dim };
    p.text(egui::pos2(c.x, r.bottom() - 12.0), egui::Align2::CENTER_CENTER, crate::i18n::tr(label), t.font(14.0), text);
    enabled && resp.clicked()
}

/// A row of the list under the buttons: icon, label, and a gear at the right. (Tapped, gear tapped.)
fn list_row(ui: &mut egui::Ui, id: &str, icon: Icon, label: &str, enabled: bool) -> (bool, bool) {
    let t = Tokens::get(ui.ctx());
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(vec2(w, 52.0), if enabled { Sense::click() } else { Sense::hover() });
    let gear_r = Rect::from_center_size(egui::pos2(r.right() - 20.0, r.center().y), vec2(40.0, 40.0));
    let gear = ui.interact(gear_r, ui.scope_id().with((id, "gear")), Sense::click());
    register(ui, format!("button:{id}"), r);
    register(ui, format!("button:{id}Settings"), gear_r);
    let p = ui.painter();
    let color = if enabled { t.text } else { t.text_dim };
    paint(p, Rect::from_center_size(egui::pos2(r.left() + 14.0, r.center().y), vec2(22.0, 22.0)), icon, color);
    p.text(egui::pos2(r.left() + 40.0, r.center().y), egui::Align2::LEFT_CENTER, crate::i18n::tr(label), t.font(16.0), color);
    paint(p, Rect::from_center_size(gear_r.center(), vec2(20.0, 20.0)), Icon::Gear, t.text);
    let on_gear = gear.clicked();
    (enabled && resp.clicked() && !on_gear, on_gear)
}

/// "Add border and share": a framed square for an icon, the label and ›. True when tapped.
fn border_row(ui: &mut egui::Ui, enabled: bool) -> bool {
    let t = Tokens::get(ui.ctx());
    let w = ui.available_width();
    let (r, resp) = ui.allocate_exact_size(vec2(w, 52.0), if enabled { Sense::click() } else { Sense::hover() });
    register(ui, "button:shareBorder", r);
    let p = ui.painter();
    let color = if enabled { t.text } else { t.text_dim };
    let icon = Rect::from_center_size(egui::pos2(r.left() + 14.0, r.center().y), vec2(20.0, 20.0));
    p.rect_stroke(icon, 2.0, Stroke::new(1.5, color), StrokeKind::Inside);
    p.rect_filled(icon.shrink(5.0), 1.0, color);
    p.text(egui::pos2(r.left() + 40.0, r.center().y), egui::Align2::LEFT_CENTER, crate::i18n::tr("Add border"), t.font(16.0), color);
    paint(p, Rect::from_center_size(egui::pos2(r.right() - 12.0, r.center().y), vec2(16.0, 16.0)), Icon::ChevronRight, t.text_dim);
    enabled && resp.clicked()
}

/// Add border: the first chosen photo framed as it will be, then the colour, the thickness and
/// the canvas's shape; the round button shares the chosen photos with that border.
fn border_page(app: &mut LightcraftApp, ui: &mut egui::Ui, sheet: &mut Sheet, form: &mut BorderForm) {
    let t = Tokens::get(ui.ctx());
    let ids: Vec<PhotoId> = sheet.ids.iter().zip(&sheet.chosen).filter(|(_, c)| **c).map(|(id, _)| *id).collect();
    let mut share = false;
    let back = head(ui, true, crate::i18n::tr("Border options"), |ui| {
        let (r, resp) = ui.allocate_exact_size(vec2(44.0, 44.0), Sense::click());
        register(ui, "button:shareBorderShare", r);
        ui.painter().circle_filled(r.center(), 20.0, if ids.is_empty() { t.divider } else { t.accent });
        paint(ui.painter(), Rect::from_center_size(r.center(), vec2(20.0, 20.0)), Icon::Share, egui::Color32::WHITE);
        share = resp.clicked() && !ids.is_empty();
    });
    if back {
        sheet.page = Page::Main;
        return;
    }
    border_preview(app, ui, ids.first().copied(), form);
    ui.add_space(8.0);
    ui.label(egui::RichText::new(crate::i18n::tr("Color")).font(t.font(14.0)).color(t.text_dim));
    egui::ScrollArea::horizontal().id_salt("border-colors").scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(
        ui,
        |ui| {
            ui.horizontal(|ui| {
                for (i, c) in BORDER_COLORS.iter().enumerate() {
                    let (r, resp) = ui.allocate_exact_size(vec2(44.0, 44.0), Sense::click());
                    register(ui, format!("button:borderColor{i}"), r);
                    let p = ui.painter();
                    p.circle(r.center(), 14.0, egui::Color32::from_rgb(c[0], c[1], c[2]), Stroke::new(1.0, t.divider));
                    if form.color == *c {
                        p.circle_stroke(r.center(), 19.0, Stroke::new(2.5, t.text));
                    }
                    if resp.clicked() {
                        form.color = *c;
                    }
                }
            });
        },
    );
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(crate::i18n::tr("Thickness")).font(t.font(14.0)).color(t.text_dim));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(form.thickness.to_string()).font(t.font(14.0)).color(t.text));
        });
    });
    ui.spacing_mut().slider_width = ui.available_width() - 8.0;
    let slider = ui.add_enabled(form.shape > 0, egui::Slider::new(&mut form.thickness, 0..=BORDER_STEPS).show_value(false));
    register(ui, "slider:borderThickness", slider.rect);
    ui.add_space(6.0);
    ui.label(egui::RichText::new(crate::i18n::tr("Border size")).font(t.font(14.0)).color(t.text_dim));
    egui::ScrollArea::horizontal().id_salt("border-shapes").scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden).show(
        ui,
        |ui| {
            ui.horizontal(|ui| {
                for (i, (label, ratio)) in BORDER_SHAPES.iter().enumerate() {
                    let (r, resp) = ui.allocate_exact_size(vec2(58.0, 70.0), Sense::click());
                    register(ui, format!("button:borderShape{i}"), r);
                    let on = form.shape == i;
                    let p = ui.painter();
                    let well = Rect::from_min_size(r.min + vec2(7.0, 2.0), vec2(44.0, 44.0));
                    p.rect(
                        well,
                        6.0,
                        if on { t.tool_active } else { t.field },
                        Stroke::new(1.0, if on { t.accent } else { t.field_border }),
                        StrokeKind::Inside,
                    );
                    let color = if on { t.text } else { t.icon };
                    match (i, ratio) {
                        (0, _) => paint(p, well.shrink(13.0), Icon::Close, color),
                        (_, None) => {
                            p.rect_stroke(well.shrink(9.0), 1.0, Stroke::new(1.5, color), StrokeKind::Inside);
                            p.rect_stroke(well.shrink(14.0), 0.0, Stroke::new(1.0, color), StrokeKind::Inside);
                        }
                        (_, Some([w, h])) => {
                            let k = 26.0 / f32::from((*w).max(*h)).max(1.0);
                            let shape = Rect::from_center_size(well.center(), vec2(f32::from(*w) * k, f32::from(*h) * k));
                            p.rect_stroke(shape, 1.0, Stroke::new(1.5, color), StrokeKind::Inside);
                        }
                    }
                    p.text(
                        egui::pos2(r.center().x, r.bottom() - 10.0),
                        egui::Align2::CENTER_CENTER,
                        crate::i18n::tr(label),
                        t.font(12.5),
                        if on { t.text } else { t.text_label },
                    );
                    if resp.clicked() {
                        form.shape = i;
                    }
                }
            });
        },
    );
    if share {
        let params = form.apply(Form::default().params(&ids, None));
        run_params(app, ui.ctx(), params, Handoff::Share);
    }
}

/// The photo as Add border will frame it: the border's colour and shape round the thumbnail.
fn border_preview(app: &mut LightcraftApp, ui: &mut egui::Ui, id: Option<PhotoId>, form: &BorderForm) {
    let t = Tokens::get(ui.ctx());
    let avail = ui.available_width();
    let area_h = (avail * 0.75).min(ui.ctx().content_rect().height() * 0.32);
    let (area, _) = ui.allocate_exact_size(vec2(avail, area_h), Sense::hover());
    register(ui, "image:borderPreview", area);
    let Some(id) = id else { return };
    let ppp = ui.ctx().pixels_per_point();
    super::grid::request_thumb(app, id, (area_h * ppp) as usize, 10);
    let Some(tex) = app.renderer.thumb(id) else {
        ui.painter().rect_filled(area, 4.0, t.divider);
        return;
    };
    let [w, h] = tex.size;
    let (cw, ch, x0, y0) = form.border().map_or((w, h, 0, 0), |b| b.layout(w, h));
    let scale = (area.width() / cw.max(1) as f32).min(area.height() / ch.max(1) as f32);
    let canvas = Rect::from_center_size(area.center(), vec2(cw as f32 * scale, ch as f32 * scale));
    let [r, g, b] = form.color;
    // (outlined: a dark border on the dark sheet still shows its edge)
    ui.painter().rect(canvas, 0.0, egui::Color32::from_rgb(r, g, b), Stroke::new(1.0, t.divider), StrokeKind::Outside);
    let photo = Rect::from_min_size(canvas.min + vec2(x0 as f32 * scale, y0 as f32 * scale), vec2(w as f32 * scale, h as f32 * scale));
    let uv = Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
    ui.painter().image(tex.tex.id(), photo, uv, egui::Color32::WHITE);
}

/// Export as / Save to Photos settings; ✓ runs it (Export as), or keeps them (Save to Photos).
fn form_page(app: &mut LightcraftApp, ui: &mut egui::Ui, sheet: &mut Sheet, form: &mut Form, to: Handoff) {
    let t = Tokens::get(ui.ctx());
    let ids: Vec<PhotoId> = sheet.ids.iter().zip(&sheet.chosen).filter(|(_, c)| **c).map(|(id, _)| *id).collect();
    let title = match to {
        Handoff::Share => crate::i18n::tr_format!("Export as… ({n})", n = ids.len()),
        Handoff::Photos => crate::i18n::tr("Save to Photos").to_string(),
    };
    let mut done = false;
    let back = head(ui, true, &title, |ui| {
        done = icon_button(ui, "shareDone", Icon::Check, vec2(40.0, 40.0), false, !ids.is_empty() || to == Handoff::Photos, "Done").clicked();
    });
    if back {
        sheet.page = Page::Main;
        return;
    }
    choice(ui, "shareFormat", "File type", FORMATS, &mut form.format);
    if form.rendered() {
        choice(ui, "shareSize", "Dimension", SIZES, &mut form.long_edge);
        if form.lossy() {
            let labels: Vec<(u8, String)> = QUALITIES.iter().map(|q| (*q, format!("{q}%"))).collect();
            let refs: Vec<(u8, &str)> = labels.iter().map(|(q, s)| (*q, s.as_str())).collect();
            choice(ui, "shareQuality", "Image quality", &refs, &mut form.quality);
        }
        ui.add_space(6.0);
        let saved = watermark(app).is_some();
        ui.horizontal(|ui| {
            ui.set_height(44.0);
            ui.label(egui::RichText::new(crate::i18n::tr("Include watermark")).font(t.font(16.0)).color(if saved { t.text } else { t.text_dim }));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let mut on = form.watermark && saved;
                let r = ui.add_enabled(saved, egui::Checkbox::without_text(&mut on));
                register(ui, "toggle:shareWatermark", r.rect);
                if r.changed() {
                    form.watermark = on;
                }
            });
        });
        if !saved {
            ui.label(egui::RichText::new(crate::i18n::tr("Set up a watermark in More options first.")).font(t.font(13.0)).color(t.text_dim));
        }
    }
    ui.add_space(6.0);
    ui.separator();
    let more = ui.add_sized(
        vec2(ui.available_width(), 48.0),
        egui::Button::new(egui::RichText::new(crate::i18n::tr("More options")).font(t.font(16.0)).color(t.text)).frame(false),
    );
    register(ui, "button:shareMoreOptions", more.rect);
    paint(
        ui.painter(),
        Rect::from_center_size(egui::pos2(more.rect.right() - 12.0, more.rect.center().y), vec2(16.0, 16.0)),
        Icon::ChevronRight,
        t.text,
    );
    if more.clicked() {
        app.session.selection.ids = ids.clone();
        let _ = app.run("dialog.export", json!({}));
        close(ui.ctx());
        return;
    }
    if done {
        match to {
            Handoff::Share => run(app, ui.ctx(), form, &ids, Handoff::Share),
            Handoff::Photos => sheet.page = Page::Main,
        }
    }
}

/// A labelled dropdown over `options`.
fn choice<T: Copy + PartialEq>(ui: &mut egui::Ui, id: &str, label: &str, options: &[(T, &str)], value: &mut T) {
    let t = Tokens::get(ui.ctx());
    ui.add_space(6.0);
    ui.label(egui::RichText::new(crate::i18n::tr(label)).font(t.font(14.0)).color(t.text_dim));
    let current = options.iter().find(|(v, _)| v == value).map_or("", |(_, l)| *l);
    let r = crate::widgets::combo_box(id)
        .width(ui.available_width())
        .selected_text(egui::RichText::new(crate::i18n::tr(current)).font(t.font(16.0)))
        .show_ui(ui, |ui| {
            for (v, l) in options {
                ui.selectable_value(value, *v, egui::RichText::new(crate::i18n::tr(l)).font(t.font(16.0)));
            }
        });
    crate::widgets::name_combo(&r.response, label);
    register(ui, format!("combo:{id}"), r.response.rect);
}

/// The watermark set in the full Export dialog (the last export of that dialog), if any.
fn watermark(app: &LightcraftApp) -> Option<Value> {
    app.session.last_export.as_ref().and_then(|l| l.get("watermark")).filter(|w| w.is_object() || w.is_string()).cloned()
}

/// Export `ids` with `form` in the background; `to` takes the files once written. Export with
/// Previous keeps what it had: the sheet's exports are one-offs.
fn run(app: &mut LightcraftApp, ctx: &egui::Context, form: &Form, ids: &[PhotoId], to: Handoff) {
    if ids.is_empty() {
        return;
    }
    let params = form.params(ids, watermark(app).as_ref());
    run_params(app, ctx, params, to);
}

/// Run `app.export` with `params` in the background, handing its files `to` the share sheet or the
/// photo library; Export with Previous keeps what it had.
fn run_params(app: &mut LightcraftApp, ctx: &egui::Context, params: Value, to: Handoff) {
    let previous = app.session.last_export.clone();
    app.export_handoff = Some(to);
    let r = app.run("app.export", params);
    app.export_handoff = None;
    app.session.last_export = previous;
    let _ = app.session.save_prefs();
    match r {
        Ok(_) => close(ctx),
        Err(e) => app.toast(ctx, e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn share_is_a_full_size_srgb_jpeg() {
        let p = Form::default().params(&[PhotoId(3), PhotoId(5)], None);
        assert_eq!(
            p,
            json!({"format": "jpeg", "ids": [3, 5], "background": true, "longEdge": 0, "dontEnlarge": true, "colorSpace": "srgb", "quality": 92})
        );
    }

    #[test]
    fn add_border_adds_the_export_border() {
        let p = BorderForm::default().apply(Form::default().params(&[PhotoId(1)], None));
        assert_eq!(p["border"], json!({"color": [255, 255, 255], "thickness": 0.05, "ratio": null}));
        let square = BorderForm { color: [0, 0, 0], thickness: 2, shape: 2 };
        assert_eq!(square.apply(json!({}))["border"], json!({"color": [0, 0, 0], "thickness": 0.02, "ratio": [1, 1]}));
        let none = BorderForm { shape: 0, ..Default::default() };
        assert!(none.apply(json!({})).get("border").is_none(), "Original: no border");
        // every shape the sheet offers is one the export draws
        for (i, (label, ratio)) in BORDER_SHAPES.iter().enumerate().skip(1) {
            let b = BorderForm { shape: i, ..Default::default() }.border().expect("a border");
            assert_eq!(b.ratio, *ratio);
            assert!(ratio.is_none() || b.shape().is_some(), "{label}");
        }
    }

    #[test]
    fn originals_and_dngs_take_no_rendering_settings() {
        for format in ["original", "dng"] {
            let f = Form { format, long_edge: 2048, quality: 70, watermark: true };
            assert_eq!(f.params(&[PhotoId(1)], Some(&json!("© me"))), json!({"format": format, "ids": [1], "background": true}));
        }
    }

    #[test]
    fn quality_only_for_lossy_formats_and_watermark_only_when_one_is_set() {
        let tif = Form { format: "tiff", long_edge: 1080, quality: 70, watermark: true };
        let p = tif.params(&[PhotoId(1)], None);
        assert!(p.get("quality").is_none() && p.get("watermark").is_none(), "{p}");
        let p = tif.params(&[PhotoId(1)], Some(&json!({"text": "© me"})));
        assert_eq!(p["watermark"], json!({"text": "© me"}));
        assert_eq!(p["longEdge"], 1080);
    }
}
