//! Touch marks for screen recordings (Settings ▸ Interface ▸ Show touches).

/// Settings ▸ Interface ▸ Show touches: screen recordings don't show where or when a finger was,
/// so each touch draws a dot while down and a ring for half a second after it lifts.
pub(crate) fn show_touches(ctx: &egui::Context) {
    const KEEP: f64 = 0.5;
    let id = egui::Id::unique("lc-show-touches");
    let now = ctx.input(|i| i.time);
    let mut marks: Vec<(u64, egui::Pos2, Option<f64>)> = ctx.data(|d| d.get_temp(id)).unwrap_or_default();
    ctx.input(|i| {
        for e in &i.events {
            if let egui::Event::Touch { id, phase, pos, .. } = e {
                marks.retain(|m| m.0 != id.0 || m.2.is_none() || *phase != egui::TouchPhase::Start);
                match phase {
                    egui::TouchPhase::Start => marks.push((id.0, *pos, None)),
                    egui::TouchPhase::Move => marks.iter_mut().filter(|m| m.0 == id.0 && m.2.is_none()).for_each(|m| m.1 = *pos),
                    _ => marks.iter_mut().filter(|m| m.0 == id.0 && m.2.is_none()).for_each(|m| m.2 = Some(now)),
                }
            }
        }
    });
    marks.retain(|m| m.2.is_none_or(|t| now - t < KEEP));
    let p = ctx.layer_painter(egui::LayerId::new(egui::Order::Debug, id));
    for (_, pos, up) in &marks {
        match up {
            None => {
                p.circle_filled(*pos, 22.0, egui::Color32::from_rgba_unmultiplied(255, 64, 64, 150));
            }
            // (no frames asked for: a ring left on screen is a frame the app didn't draw)
            Some(_) => {
                p.circle_stroke(*pos, 28.0, egui::Stroke::new(3.0, egui::Color32::from_rgb(255, 64, 64)));
            }
        }
    }
    ctx.data_mut(|d| d.insert_temp(id, marks));
}
