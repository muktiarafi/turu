//! Mugetsu's pacing for frame interpolation (`ruffle_core::frame_interpolation`), shared by the
//! desktop window (`app.rs`) and Android's screen. With interpolation on, the screen is presented
//! on every refresh of the display: presenting waits for vsync, which paces this exactly (a timer
//! is ~1-2 ms late, a lot at 120 Hz and more). A new in-between picture is drawn on every n-th of
//! them, the most the thread can afford, so each stays up the same time; the others show the
//! last one again, which costs little. It needs at least 1.5 new pictures per SWF frame, else the
//! screen is drawn once per frame as without interpolation.
//!
//! The frontend tells it what SWF frames and draws cost ([`Pacing::record_frame`],
//! [`Pacing::record_draw`]) and the display's refresh interval; [`Pacing::plan`] after each pass
//! says whether to interpolate, and before each draw [`Pacing::new_picture_due`] whether to draw a
//! new picture or show the last one again.

use std::time::{Duration, Instant};

pub struct Pacing {
    /// Recent averages of a draw's and a SWF frame's time on the player's thread (ms).
    draw_cost_ms: f64,
    frame_cost_ms: f64,
    /// Draw a new picture on every n-th refresh of the display.
    draw_divisor: u32,
    /// While interpolating: when the last new picture was due.
    picture_due: Instant,
    /// Since when `draw_divisor` would have gone up / down (it changes only if that lasts).
    divisor_up_since: Option<Instant>,
    divisor_down_since: Option<Instant>,
    /// The display's refresh interval.
    refresh_interval: Duration,
    /// The new pictures per second aimed for, while interpolating.
    rate: f64,
}

impl Default for Pacing {
    fn default() -> Self {
        Self::new()
    }
}

impl Pacing {
    pub fn new() -> Self {
        Self {
            draw_cost_ms: 2.0,
            frame_cost_ms: 3.0,
            draw_divisor: 1,
            picture_due: Instant::now(),
            divisor_up_since: None,
            divisor_down_since: None,
            refresh_interval: Duration::from_secs_f64(1.0 / 60.0),
            rate: 0.0,
        }
    }

    pub fn refresh_interval(&self) -> Duration {
        self.refresh_interval
    }

    /// The display refreshes `millihertz` times a thousand seconds (24 to 360 Hz are believed).
    /// Returns whether that changed.
    pub fn set_refresh_rate(&mut self, millihertz: u32) -> bool {
        let interval = Duration::from_secs_f64(1000.0 / millihertz.clamp(24_000, 360_000) as f64);
        let changed = interval != self.refresh_interval;
        self.refresh_interval = interval;
        changed
    }

    /// The new pictures per second aimed for, while interpolating (for the perf log).
    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// A tick that ran SWF frames took `frame_ms`. One-off slow frames, e.g. a room loading,
    /// count at most 3x the average (as draws do): one 300 ms frame made interpolation think every
    /// frame cost 30 ms and drop to once per frame for seconds.
    pub fn record_frame(&mut self, frame_ms: f64) {
        let cost = frame_ms.min(self.frame_cost_ms * 3.0);
        self.frame_cost_ms = self.frame_cost_ms * 0.9 + cost * 0.1;
    }

    /// Drawing the movie (only: presenting can include waiting for the refresh) took `movie_ms`;
    /// one-off slow draws, e.g. of new shapes, count at most 3x the average.
    pub fn record_draw(&mut self, movie_ms: f64) {
        let cost = movie_ms.min(self.draw_cost_ms * 3.0);
        self.draw_cost_ms = self.draw_cost_ms * 0.9 + cost * 0.1;
    }

    /// While interpolating, for a draw that started at `started`: whether it draws a new
    /// in-between picture. One goes out on the first refresh after it's due, every n refreshes'
    /// time; the refreshes in between show the last one again. Timed, not counted: a long draw
    /// (e.g. 11 ms in Yulgar) makes presents miss refreshes.
    pub fn new_picture_due(&mut self, started: Instant) -> bool {
        let interval = self.refresh_interval * self.draw_divisor;
        let due = self.picture_due + interval;
        if started + self.refresh_interval / 2 >= due {
            // Keep the rhythm, unless it fell more than a picture behind.
            self.picture_due = if started < due + interval {
                due
            } else {
                started
            };
            true
        } else {
            false
        }
    }

    /// With interpolation on and the movie playing at `frame_rate`: whether to present on every
    /// refresh, drawing a new picture on every n-th (n is chosen here). False when not even 1.5
    /// new pictures per SWF frame fit: then draw once per frame, as without interpolation.
    pub fn plan(&mut self, now: Instant, frame_rate: f64) -> bool {
        // Draw on every refresh if the thread can afford it (keeping a third of its time spare),
        // else on every 2nd, 3rd, ... refresh: an even rate looks smoother than an uneven higher
        // one. Each change of rate is a visible change of rhythm, so it only steps to fewer
        // pictures after needing to for half a second, and back to more after 3 seconds of
        // clearly affording it; 1 second for a big step back (at least twice the pictures, or out
        // of drawing once per frame), as after a room has loaded.
        let refresh_hz = 1.0 / self.refresh_interval.as_secs_f64();
        let spare_ms = (650.0 - self.frame_cost_ms * frame_rate).max(100.0);
        let divisor_for = |cost_ms: f64| {
            (refresh_hz * cost_ms.max(0.5) / spare_ms)
                .ceil()
                .clamp(1.0, 16.0) as u32
        };
        let needed = divisor_for(self.draw_cost_ms);
        let lasted = |since: &mut Option<Instant>, wanted: bool, secs: f64| {
            if !wanted {
                *since = None;
                return false;
            }
            now.duration_since(*since.get_or_insert(now)).as_secs_f64() >= secs
        };
        let affordable = divisor_for(self.draw_cost_ms * 1.3);
        let once_per_frame =
            (self.refresh_interval * self.draw_divisor).as_secs_f64() * 1.5 > 1.0 / frame_rate;
        let big_step = once_per_frame || affordable * 2 <= self.draw_divisor;
        if lasted(&mut self.divisor_up_since, needed > self.draw_divisor, 0.5)
            || lasted(
                &mut self.divisor_down_since,
                affordable < self.draw_divisor,
                if big_step { 1.0 } else { 3.0 },
            )
        {
            self.draw_divisor = needed;
            self.divisor_up_since = None;
            self.divisor_down_since = None;
        }
        let interval = self.refresh_interval * self.draw_divisor;
        self.rate = refresh_hz / self.draw_divisor as f64;
        // Not even 1.5 draws per SWF frame: draw once per frame, as without interpolation.
        interval.as_secs_f64() * 1.5 <= 1.0 / frame_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cheap_draws_interpolate_on_every_refresh() {
        let mut pacing = Pacing::new();
        pacing.set_refresh_rate(120_000);
        let now = Instant::now();
        assert!(pacing.plan(now, 24.0));
        assert_eq!(pacing.draw_divisor, 1);
        assert!((pacing.rate() - 120.0).abs() < 0.01);
    }

    #[test]
    fn costly_draws_step_down_only_after_half_a_second() {
        let mut pacing = Pacing::new();
        pacing.set_refresh_rate(120_000);
        for _ in 0..100 {
            pacing.record_draw(8.0);
        }
        let start = Instant::now();
        assert!(pacing.plan(start, 24.0));
        assert_eq!(pacing.draw_divisor, 1, "not yet: it has to last");
        assert!(pacing.plan(start + Duration::from_millis(600), 24.0));
        assert!(pacing.draw_divisor > 1);
    }

    #[test]
    fn too_few_pictures_per_frame_draw_once_per_frame() {
        let mut pacing = Pacing::new();
        // 30 Hz, a 24 fps movie: 1.25 pictures per frame.
        pacing.set_refresh_rate(30_000);
        assert!(!pacing.plan(Instant::now(), 24.0));
    }

    #[test]
    fn new_pictures_keep_their_rhythm() {
        let mut pacing = Pacing::new();
        pacing.set_refresh_rate(60_000);
        let start = pacing.picture_due;
        let refresh = pacing.refresh_interval();
        assert!(!pacing.new_picture_due(start), "the last one was just due");
        assert!(pacing.new_picture_due(start + refresh));
        assert!(!pacing.new_picture_due(start + refresh + refresh / 4));
        assert!(pacing.new_picture_due(start + refresh * 2));
    }
}
