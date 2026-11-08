//! The phone layout's Rate sheet: a photo's (or the selection's) star rating, flag and colour
//! label in one sheet with finger-sized controls, instead of three levels of All Menus. Each tap
//! runs `photo.rate`, `photo.flag` or `photo.label` on the photos it was opened for (never moving
//! the selection on); the sheet shows the first photo's values. Opened by ★ in the editor's bar and
//! in Select mode's bar ([`super::compact`]).

use egui::{Stroke, vec2};
use lightcraft_catalog::{ColorLabel, Flag, PhotoId};
use serde_json::{Value, json};

use super::filter_sheet::{caption, head, pill, pills, wide_button};
use crate::LightcraftApp;
use crate::icons::{Icon, paint};
use crate::theme::{Tokens, label_color};
use crate::widgets::register;

fn sheet_id() -> egui::Id {
    egui::Id::unique("compact-rate-sheet")
}

/// Open the sheet for `ids` (none: nothing happens).
pub fn open(ctx: &egui::Context, ids: Vec<PhotoId>) {
    if !ids.is_empty() {
        ctx.data_mut(|d| d.insert_temp(sheet_id(), ids));
    }
}

pub fn is_open(ctx: &egui::Context) -> bool {
    ctx.data(|d| d.get_temp::<Vec<PhotoId>>(sheet_id()).is_some())
}

fn close(ctx: &egui::Context) {
    ctx.data_mut(|d| d.remove::<Vec<PhotoId>>(sheet_id()));
}

/// Draw the sheet when it's open: at the bottom of the screen over a dimmed backdrop.
pub fn show(app: &mut LightcraftApp, ctx: &egui::Context) {
    let Some(ids) = ctx.data(|d| d.get_temp::<Vec<PhotoId>>(sheet_id())) else { return };
    let t = Tokens::get(ctx);
    let screen = ctx.content_rect();
    let width = screen.width().min(560.0);
    let area = egui::Area::new(egui::Id::unique("compact-rate-area"))
        .anchor(egui::Align2::CENTER_BOTTOM, vec2(0.0, 0.0))
        .default_width(width)
        .order(egui::Order::Foreground);
    let frame = egui::Frame::NONE
        .fill(t.chrome)
        .stroke(Stroke::new(1.0, t.divider))
        .corner_radius(egui::CornerRadius { nw: 14, ne: 14, sw: 0, se: 0 })
        .inner_margin(egui::Margin { left: 16, right: 16, top: 6, bottom: 16 });
    let mut done = false;
    let modal = egui::Modal::new(egui::Id::unique("compact-rate-modal")).area(area).frame(frame).show(ctx, |ui| {
        ui.set_width(width - 32.0);
        register(ui, "panel:rateSheet", ui.max_rect());
        done = page(app, ui, &ids);
    });
    if done || modal.should_close() {
        close(ctx);
    }
}

/// Run `cmd` on the sheet's photos (they stay selected: no advancing).
fn run(app: &mut LightcraftApp, ui: &egui::Ui, cmd: &str, mut params: Value, ids: &[PhotoId]) {
    params["ids"] = json!(ids.iter().map(|id| id.0).collect::<Vec<_>>());
    params["advance"] = json!(false);
    if let Err(e) = app.run(cmd, params) {
        app.toast(ui.ctx(), e);
    }
}

/// The sheet's controls; true when it should close.
fn page(app: &mut LightcraftApp, ui: &mut egui::Ui, ids: &[PhotoId]) -> bool {
    let t = Tokens::get(ui.ctx());
    let Some(first) = ids.first().and_then(|id| app.session.catalog.photo(*id)).cloned() else { return true };
    let title = if ids.len() == 1 { crate::i18n::tr("Rate photo").to_string() } else { crate::i18n::tr_format!("Rate {n} photos", n = ids.len()) };
    let closed = head(ui, false, &title, |_| {});

    caption(ui, "Rating");
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 4.0;
        for n in 1..=5u8 {
            let (r, resp) = ui.allocate_exact_size(vec2(48.0, 48.0), egui::Sense::click());
            register(ui, format!("rate:star-{n}"), r);
            crate::widgets::announce_selected(&resp, format!("{n} ★"), n == first.rating);
            let lit = n <= first.rating;
            paint(ui.painter(), r.shrink(9.0), if lit { Icon::StarFilled } else { Icon::Star }, if lit { t.text } else { t.icon });
            if resp.clicked() {
                // the lit last star again: no rating
                run(app, ui, "photo.rate", json!({"rating": if n == first.rating { 0 } else { n }}), ids);
            }
        }
    });

    caption(ui, "Flag");
    pills(ui, |ui| {
        for (key, icon, flag, label) in [
            ("pick", Icon::FlagPickFilled, Flag::Pick, "Picked"),
            ("reject", Icon::FlagReject, Flag::Reject, "Rejected"),
            ("none", Icon::Circle, Flag::None, "Unflagged"),
        ] {
            let color = match flag {
                Flag::Pick => t.pick,
                Flag::Reject => t.reject,
                Flag::None => t.icon,
            };
            if pill(ui, &format!("rate:flag-{key}"), Some((icon, color)), label, first.flag == flag) {
                run(app, ui, "photo.flag", json!({"flag": key}), ids);
            }
        }
    });

    caption(ui, "Color label");
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = 6.0;
        let none = std::iter::once((None, "none".to_string()));
        let labels = ColorLabel::ALL.into_iter().map(|l| (Some(l), format!("{l:?}").to_lowercase()));
        for (label, key) in none.chain(labels) {
            let (r, resp) = ui.allocate_exact_size(vec2(44.0, 44.0), egui::Sense::click());
            register(ui, format!("rate:label-{key}"), r);
            let name = label.map_or_else(|| crate::i18n::tr("No label").to_string(), |l| crate::i18n::color_label(&app.session.catalog, l));
            let on = first.label == label;
            crate::widgets::announce_selected(&resp, name, on);
            let p = ui.painter();
            match label {
                Some(l) => {
                    p.circle_filled(r.center(), 14.0, label_color(l));
                }
                None => {
                    p.circle_stroke(r.center(), 13.0, Stroke::new(1.5, t.icon));
                    p.line_segment([r.center() + vec2(-9.0, 9.0), r.center() + vec2(9.0, -9.0)], Stroke::new(1.5, t.icon));
                }
            }
            if on {
                p.circle_stroke(r.center(), 19.0, Stroke::new(2.5, t.text));
            }
            if resp.clicked() && !on {
                run(app, ui, "photo.label", json!({"label": key}), ids);
            }
        }
    });
    ui.add_space(16.0);
    closed | wide_button(ui, "rateSheetDone", crate::i18n::tr("Done"), true, true)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_sheet_opens_for_photos_only() {
        let ctx = egui::Context::default();
        open(&ctx, Vec::new());
        assert!(!is_open(&ctx), "nothing to rate");
        open(&ctx, vec![PhotoId(1)]);
        assert!(is_open(&ctx));
        close(&ctx);
        assert!(!is_open(&ctx));
    }
}
