//! Swiping from photo to photo with a finger, as phone photo apps have it: the photo follows the
//! finger with its neighbour sliding in beside it, a gap between them. Let go past half the way,
//! or flicked, it carries on to the neighbour; otherwise it slides back. Past the first or last
//! photo it gives less and less. A finger that catches a photo still sliding takes it from there.
//!
//! The state lives in the context's memory, shared by the loupe and the sideways view (only one
//! shows at a time). `offset` is always the active photo's: once a swipe has stepped, the new
//! photo starts from where it was on screen as the neighbour, and slides home.

use egui::{Pos2, Vec2};
use std::collections::VecDeque;

/// Between a photo and its neighbour while they move (pt).
pub(crate) const GAP: f32 = 24.0;
/// A finger lifted faster than this (pt/s) along the swipe carries on to the neighbour.
const FLICK: f32 = 350.0;
/// The finger moves this far (pt) before the swipe decides whether it is sideways.
const SLOP: f32 = 8.0;
/// The slide home, after a lift: the fastest and slowest (s).
const SETTLE_SECS: (f32, f32) = (0.16, 0.34);
/// The finger's samples kept for its speed at the lift (s).
const SPEED_WINDOW: f64 = 0.08;

/// Which way a swipe stepped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Step {
    Next,
    Previous,
}

#[derive(Clone, Default)]
struct State {
    /// How far the active photo is off its place along the swipe (pt; + towards the previous).
    offset: f32,
    finger: Option<Finger>,
    settle: Option<Settle>,
}

#[derive(Clone)]
struct Finger {
    origin: Pos2,
    /// The offset when the finger came down (a photo caught mid-slide).
    base: f32,
    /// Sideways (`Some(true)`), something else (`Some(false)`) or not yet known.
    sideways: Option<bool>,
    /// Recent (time, distance along) for the speed at the lift.
    samples: VecDeque<(f64, f32)>,
    /// The way it goes, as last tracked.
    track: Track,
}

impl Finger {
    /// The offset with the finger at `pos`.
    fn offset_at(&self, pos: Pos2) -> f32 {
        let t = &self.track;
        let raw = self.base + (t.axis)(pos - self.origin, t.turn).0;
        give(raw, t.pitch, if raw > 0.0 { t.has_previous } else { t.has_next })
    }
}

#[derive(Clone, Copy)]
struct Settle {
    from: f32,
    start: f64,
    secs: f32,
}

impl State {
    fn at(&self, now: f64) -> f32 {
        match self.settle {
            Some(s) => {
                let t = (((now - s.start) as f32) / s.secs).clamp(0.0, 1.0);
                // ease out (cubic): fast at first, matching the finger, then gently home
                s.from * (1.0 - t).powi(3)
            }
            None => self.offset,
        }
    }

    fn settled(&self, now: f64) -> bool {
        self.finger.is_none() && self.settle.is_none_or(|s| now - s.start >= f64::from(s.secs))
    }
}

fn id() -> egui::Id {
    egui::Id::unique("loupe-swipe")
}

fn load(ctx: &egui::Context) -> State {
    ctx.data(|d| d.get_temp::<State>(id())).unwrap_or_default()
}

fn store(ctx: &egui::Context, s: State) {
    ctx.data_mut(|d| d.insert_temp(id(), s));
}

/// How far the active photo is off its place along the swipe now (0 at rest); keeps the frames
/// coming while it slides.
pub(crate) fn offset(ctx: &egui::Context) -> f32 {
    let now = ctx.input(|i| i.time);
    let s = load(ctx);
    // a finger swiping: where it is this frame (the swipe is tracked after the photo is laid out,
    // which would leave the photo a frame behind the finger)
    if let Some(f) = s.finger.as_ref().filter(|f| f.sideways == Some(true))
        && let Some(pos) = ctx.input(|i| i.pointer.interact_pos().filter(|_| i.pointer.any_down()))
    {
        return f.offset_at(pos);
    }
    if s.settle.is_some() {
        if s.settled(now) {
            store(ctx, State::default());
            return 0.0;
        }
        ctx.request_repaint();
    }
    s.at(now)
}

/// Drop the swipe (a pinch or two-finger pan took over): the photo is back in place.
pub(crate) fn cancel(ctx: &egui::Context) {
    ctx.data_mut(|d| d.remove::<State>(id()));
}

/// Where the swipe goes on the screen, and how far it is from one photo to the next.
#[derive(Clone, Copy)]
pub(crate) struct Track {
    /// The finger's movement on the screen → (along the swipe, across it); along is + towards
    /// the previous photo.
    pub axis: fn(Vec2, i8) -> (f32, f32),
    /// Passed to `axis` (the sideways view's turn).
    pub turn: i8,
    /// From a photo's place to its neighbour's (pt).
    pub pitch: f32,
    pub has_previous: bool,
    pub has_next: bool,
}

/// Left and right, upright.
pub(crate) fn upright(d: Vec2, _: i8) -> (f32, f32) {
    (d.x, d.y)
}

/// Follow a touch swipe on `resp`: returns the step to take once a lift carries on to a
/// neighbour (the caller steps, this has already moved the offset to the new photo), and whether
/// the finger is swiping (its other uses wait). Touch only: a mouse drag is the caller's.
pub(crate) fn track(ui: &egui::Ui, resp: &egui::Response, t: &Track) -> (Option<Step>, bool) {
    let ctx = ui.ctx();
    let now = ui.input(|i| i.time);
    let mut s = load(ctx);
    if resp.drag_started()
        && ui.input(|i| i.any_touches())
        && let Some(origin) = ui.input(|i| i.pointer.press_origin())
    {
        // a photo caught while it slides: from where it is
        let base = s.at(now);
        s = State { offset: base, finger: Some(Finger { origin, base, sideways: None, samples: VecDeque::new(), track: *t }), settle: None };
    }
    let Some(f) = s.finger.as_mut() else { return (None, false) };
    f.track = *t;
    if resp.drag_stopped() {
        let swiping = f.sideways == Some(true);
        let speed = speed(&f.samples);
        let step = if swiping { lift(&mut s, speed, now, t) } else { None };
        if !swiping {
            s = State { offset: s.offset, finger: None, settle: None };
            if s.offset != 0.0 {
                s.settle = Some(Settle { from: s.offset, start: now, secs: SETTLE_SECS.1 });
            }
        }
        store(ctx, s);
        return (step, swiping);
    }
    if !resp.dragged() {
        store(ctx, s);
        return (None, false);
    }
    let Some(pos) = ui.input(|i| i.pointer.interact_pos()) else {
        let swiping = f.sideways == Some(true);
        store(ctx, s);
        return (None, swiping);
    };
    let (along, across) = (t.axis)(pos - f.origin, t.turn);
    if f.sideways.is_none() && (along.abs() > SLOP || across.abs() > SLOP) {
        f.sideways = Some(along.abs() > across.abs());
    }
    let swiping = f.sideways == Some(true);
    if swiping {
        f.samples.push_back((now, along));
        while f.samples.len() > 2 && f.samples.front().is_some_and(|(at, _)| now - at > SPEED_WINDOW) {
            f.samples.pop_front();
        }
        s.offset = f.offset_at(pos);
    }
    store(ctx, s);
    (None, swiping)
}

/// Past the end, the photo gives less the further it goes (as iOS scroll views do).
fn give(raw: f32, pitch: f32, room: bool) -> f32 {
    if room {
        return raw;
    }
    let d = pitch.max(1.0) / 2.0;
    raw.signum() * (1.0 - 1.0 / (raw.abs() * 0.55 / d + 1.0)) * d
}

/// The finger's speed along the swipe at the lift (pt/s).
fn speed(samples: &VecDeque<(f64, f32)>) -> f32 {
    match (samples.front(), samples.back()) {
        (Some(&(t0, a0)), Some(&(t1, a1))) if t1 - t0 > 0.005 => (a1 - a0) / (t1 - t0) as f32,
        _ => 0.0,
    }
}

/// The finger lifted: on to a neighbour (flicked, or let go past half the way) or back home.
fn lift(s: &mut State, speed: f32, now: f64, t: &Track) -> Option<Step> {
    let off = s.offset;
    let towards = if speed.abs() > FLICK {
        speed.signum()
    } else if off.abs() > t.pitch / 2.0 {
        off.signum()
    } else {
        0.0
    };
    // (a flick back against the way the photo went brings it home)
    let step = match towards {
        d if d < 0.0 && off <= 0.0 && t.has_next => Some(Step::Next),
        d if d > 0.0 && off >= 0.0 && t.has_previous => Some(Step::Previous),
        _ => None,
    };
    // the new photo was the neighbour, a pitch further along
    let from = match step {
        Some(Step::Next) => off + t.pitch,
        Some(Step::Previous) => off - t.pitch,
        None => off,
    };
    // the slide home starts at the finger's speed where it goes the same way (an ease-out cubic
    // starts at 3 × distance / time)
    let secs = if from != 0.0 && speed.abs() > 1.0 && speed.signum() == -from.signum() { 3.0 * from.abs() / speed.abs() } else { SETTLE_SECS.1 };
    *s = State { offset: from, finger: None, settle: Some(Settle { from, start: now, secs: secs.clamp(SETTLE_SECS.0, SETTLE_SECS.1) }) };
    step
}

#[cfg(test)]
mod tests {
    use super::*;

    fn track(prev: bool, next: bool) -> Track {
        Track { axis: upright, turn: 0, pitch: 400.0, has_previous: prev, has_next: next }
    }

    fn finger(base: f32, samples: &[(f64, f32)]) -> State {
        State {
            offset: base + samples.last().map_or(0.0, |s| s.1),
            finger: Some(Finger {
                origin: Pos2::ZERO,
                base,
                sideways: Some(true),
                samples: samples.iter().copied().collect(),
                track: track(true, true),
            }),
            settle: None,
        }
    }

    #[test]
    fn past_half_way_it_steps_and_the_new_photo_slides_home_from_where_it_was() {
        let mut s = finger(0.0, &[(0.0, -210.0), (0.08, -210.0)]);
        s.offset = -210.0;
        assert_eq!(lift(&mut s, 0.0, 1.0, &track(true, true)), Some(Step::Next));
        assert_eq!(s.at(1.0), 190.0, "the next photo was 400 pt to the right of the current one");
        assert_eq!(s.at(1.0 + f64::from(SETTLE_SECS.1)), 0.0);
    }

    #[test]
    fn a_short_slow_drag_slides_back_but_a_flick_steps() {
        let mut s = finger(0.0, &[]);
        s.offset = -80.0;
        assert_eq!(lift(&mut s, -50.0, 0.0, &track(true, true)), None);
        assert_eq!(s.at(0.0), -80.0);
        s.offset = -80.0;
        assert_eq!(lift(&mut s, -900.0, 0.0, &track(true, true)), Some(Step::Next));
        assert_eq!(lift(&mut finger(0.0, &[]), 900.0, 0.0, &track(true, true)), Some(Step::Previous));
    }

    #[test]
    fn a_flick_back_against_the_drag_brings_it_home() {
        let mut s = finger(0.0, &[]);
        s.offset = -250.0;
        assert_eq!(lift(&mut s, 900.0, 0.0, &track(true, true)), None);
    }

    #[test]
    fn past_the_last_photo_it_gives_less_and_does_not_step() {
        assert!(give(-300.0, 400.0, false).abs() < 150.0);
        assert!(give(-600.0, 400.0, false).abs() > give(-300.0, 400.0, false).abs(), "still follows, less");
        assert_eq!(give(-300.0, 400.0, true), -300.0);
        let mut s = finger(0.0, &[]);
        s.offset = give(-300.0, 400.0, false);
        assert_eq!(lift(&mut s, -900.0, 0.0, &track(true, false)), None);
    }

    #[test]
    fn the_slide_home_starts_at_the_fingers_speed() {
        let mut s = finger(0.0, &[]);
        s.offset = -150.0;
        lift(&mut s, -1500.0, 0.0, &track(true, true));
        // 250 pt to go at 1500 pt/s: 3 × 250 / 1500 = 0.5 s, clamped to the slowest
        let st = s.settle.expect("sliding");
        assert_eq!(st.secs, SETTLE_SECS.1);
        s.offset = -380.0;
        s.finger = None;
        lift(&mut s, -2000.0, 0.0, &track(true, true));
        assert!((s.settle.expect("sliding").secs - SETTLE_SECS.0).abs() < 1e-6, "20 pt at 2000 pt/s: the quickest");
    }
}
