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
//!
//! A process tab's client (`crate::client_screen`) presents nothing itself: the shell presents
//! its pictures on every refresh and tells it after each, and it asks [`Pacing::new_picture_due`]
//! then; the refreshes without a new picture show the last one again in the shell. Uncapped
//! (Mugetsu 2.0.0), each new picture goes out as soon as it's ready, at most [`fps_limit`]'s a
//! second ([`Pacing::new_uncapped_picture_due`]).

use std::time::{Duration, Instant};

/// With a render thread (Mugetsu 2.0.0's `ruffle_render_wgpu::render_thread`): the milliseconds
/// a second that thread may spend recording pictures, budgeted apart from the player's thread,
/// whose share it isn't (Mugetsu's `RENDER_THREAD_BUDGET_MS`).
const RENDER_THREAD_BUDGET_MS: f64 = 850.0;

pub struct Pacing {
    /// Recent averages of a draw's and a SWF frame's time on the player's thread (ms).
    draw_cost_ms: f64,
    frame_cost_ms: f64,
    /// With a render thread: the recent average of a picture's time on it (ms; 0 without one).
    thread_cost_ms: f64,
    /// A process tab's client: the recent average of a picture's whole way, from its draw to the
    /// GPU having finished it (ms; 0 elsewhere): with one picture under way at a time
    /// (`in_flight`), new ones can't come faster; with more it's only logged (see `plan`).
    pipeline_ms: f64,
    in_flight: u32,
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
            thread_cost_ms: 0.0,
            pipeline_ms: 0.0,
            in_flight: 1,
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

    /// The recent average of a draw's time on the player's thread (ms; see `record_draw`).
    pub fn draw_cost_ms(&self) -> f64 {
        self.draw_cost_ms
    }

    /// With a render thread, a picture took `ms` on it (its own budget, see
    /// `RENDER_THREAD_BUDGET_MS`); one-off slow ones count at most 3x the average.
    pub fn record_thread(&mut self, ms: f64) {
        let ms = if self.thread_cost_ms > 0.0 {
            ms.min(self.thread_cost_ms * 3.0)
        } else {
            ms
        };
        self.thread_cost_ms = self.thread_cost_ms * 0.9 + ms * 0.1;
    }

    /// See `thread_cost_ms` (0 without a render thread).
    pub fn thread_cost_ms(&self) -> f64 {
        self.thread_cost_ms
    }

    /// See `pipeline_ms` (0 where nothing records it).
    pub fn pipeline_ms(&self) -> f64 {
        self.pipeline_ms
    }

    /// A process tab's client: a picture took `ms` from its draw until the GPU had finished it
    /// (see `pipeline_ms`); one-off slow ones count at most 3x the average.
    pub fn record_pipeline(&mut self, ms: f64) {
        let ms = if self.pipeline_ms > 0.0 {
            ms.min(self.pipeline_ms * 3.0)
        } else {
            ms
        };
        self.pipeline_ms = self.pipeline_ms * 0.9 + ms * 0.1;
    }

    /// A process tab's client: how many pictures may be under way at once (see `pipeline_ms`).
    pub fn set_in_flight(&mut self, pictures: u32) {
        self.in_flight = pictures.max(1);
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
            self.picture_due = if started < due + interval { due } else { started };
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
        // With a render thread, its share of a picture is budgeted apart, as in Mugetsu: charged
        // to the player's thread as well, battleon's 6 ms pictures got every 3rd refresh at
        // 165 Hz, where the two threads could afford every 2nd.
        let thread_cost_ms = self.thread_cost_ms;
        let divisor_for = |cost_ms: f64, thread_margin: f64| {
            let player = refresh_hz * cost_ms.max(0.5) / spare_ms;
            let thread = if thread_cost_ms > 0.0 {
                refresh_hz * (thread_cost_ms * thread_margin).max(0.5) / RENDER_THREAD_BUDGET_MS
            } else {
                0.0
            };
            player.max(thread).ceil().clamp(1.0, 16.0) as u32
        };
        // With one picture under way at a time, its whole way has to fit between two new ones,
        // with room for a late refresh. With more, there's no such floor, as in Mugetsu: pictures
        // wait for a free slot and as many get through as the GPU allows, unevenly (Yulgar beside
        // Mugetsu on one GPU: the floor took a client to one picture per frame, 24, where ~33
        // got through and Mugetsu kept interpolating at 30-35).
        let refresh_ms = self.refresh_interval.as_secs_f64() * 1000.0;
        let pipeline_divisor = |margin: f64| {
            if self.in_flight > 1 {
                return 1;
            }
            ((self.pipeline_ms * margin / refresh_ms).ceil() as u32).clamp(1, 16)
        };
        // (No budget of the GPU's time, as in Mugetsu: what a picture takes the GPU depends on the
        // rate itself, a built-in GPU's clock falling with the load, so a budget drawn from it
        // stepped down and never came back: 10 ms a picture interpolating in battleon, 14 ms in
        // the same room at one picture per frame.)
        let needed = divisor_for(self.draw_cost_ms, 1.0).max(pipeline_divisor(1.15));
        let lasted = |since: &mut Option<Instant>, wanted: bool, secs: f64| {
            if !wanted {
                *since = None;
                return false;
            }
            now.duration_since(*since.get_or_insert(now)).as_secs_f64() >= secs
        };
        let affordable = divisor_for(self.draw_cost_ms * 1.3, 1.3).max(pipeline_divisor(1.4));
        let once_per_frame = (self.refresh_interval * self.draw_divisor).as_secs_f64() * 1.5 > 1.0 / frame_rate;
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

impl Pacing {
    /// Uncapped, at most one new picture every `interval` (the FPS limit, else none), evenly by
    /// the clock: whether a draw that started at `started` draws a new one.
    pub fn new_uncapped_picture_due(&mut self, started: Instant, interval: Option<Duration>) -> bool {
        let Some(interval) = interval else {
            return true;
        };
        let due = self.picture_due + interval;
        if started + Duration::from_micros(300) >= due {
            // Keep the rhythm, unless it fell more than a picture behind.
            self.picture_due = if started < due + interval { due } else { started };
            true
        } else {
            false
        }
    }

    /// Uncapped with an FPS limit of one picture every `interval`: when the next is due.
    pub fn next_uncapped_picture(&self, interval: Duration) -> Instant {
        self.picture_due + interval
    }
}

/// Uncapped pacing's FPS limit in pictures a second (None: no limit), from Mugetsu's setting (0:
/// none, 1: the display's rate, else that many) and the display's refresh interval. "The
/// display's rate" is three under it: at the rate itself a variable-refresh monitor stops
/// following.
pub fn fps_limit(setting: u32, refresh_interval: Duration) -> Option<f64> {
    let refresh_hz = 1.0 / refresh_interval.as_secs_f64();
    match setting {
        0 => None,
        1 => Some((refresh_hz - 3.0).max(refresh_hz * 0.9)),
        fps => Some(f64::from(fps)),
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

    #[test]
    fn uncapped_pictures_keep_the_limit() {
        let mut pacing = Pacing::new();
        let start = pacing.picture_due;
        assert!(pacing.new_uncapped_picture_due(start, None), "no limit: always");
        let interval = Duration::from_millis(10);
        assert!(!pacing.new_uncapped_picture_due(start + Duration::from_millis(5), Some(interval)));
        assert!(pacing.new_uncapped_picture_due(start + Duration::from_millis(10), Some(interval)));
        assert_eq!(
            pacing.next_uncapped_picture(interval),
            start + Duration::from_millis(20)
        );
        // Far behind: it starts again from now rather than catching up.
        let late = start + Duration::from_millis(100);
        assert!(pacing.new_uncapped_picture_due(late, Some(interval)));
        assert_eq!(pacing.next_uncapped_picture(interval), late + interval);
    }

    #[test]
    fn fps_limits() {
        let refresh = Duration::from_secs_f64(1.0 / 144.0);
        assert_eq!(fps_limit(0, refresh), None);
        assert!((fps_limit(1, refresh).unwrap() - 141.0).abs() < 0.01);
        assert_eq!(fps_limit(90, refresh), Some(90.0));
        // At 24 Hz, three under would be a big share: 90% of it instead.
        let slow = Duration::from_secs_f64(1.0 / 24.0);
        assert!((fps_limit(1, slow).unwrap() - 21.6).abs() < 0.01);
    }

    #[test]
    fn a_long_way_to_a_finished_picture_steps_down() {
        let mut pacing = Pacing::new();
        pacing.set_refresh_rate(120_000);
        // Cheap draws, but 8 ms from a draw to its finished picture: every refresh's 8.3 ms
        // leaves no room for a late one.
        for _ in 0..100 {
            pacing.record_draw(1.0);
            pacing.record_pipeline(8.0);
        }
        let start = Instant::now();
        pacing.plan(start, 30.0);
        assert!(pacing.plan(start + Duration::from_millis(600), 30.0));
        assert_eq!(pacing.draw_divisor, 2);
        // 2.5 ms fits: back to every refresh once it has lasted.
        for _ in 0..100 {
            pacing.record_pipeline(2.5);
        }
        pacing.plan(start + Duration::from_millis(700), 30.0);
        assert!(pacing.plan(start + Duration::from_millis(5000), 30.0));
        assert_eq!(pacing.draw_divisor, 1);
    }

    #[test]
    fn pictures_under_way_together_share_the_way() {
        // 165 Hz, 30 ms from a draw to its finished picture (battleon-2 on a Radeon 780M): one
        // at a time is not even 1.5 pictures per frame; with three at a time the way sets no floor
        // (as many get through as the GPU allows), so cheap draws aim for every refresh.
        let start = Instant::now();
        let run = |in_flight: u32| {
            let mut pacing = Pacing::new();
            pacing.set_refresh_rate(165_000);
            pacing.set_in_flight(in_flight);
            for _ in 0..100 {
                pacing.record_draw(1.0);
                pacing.record_pipeline(30.0);
            }
            pacing.plan(start, 24.0);
            let interpolates = pacing.plan(start + Duration::from_millis(600), 24.0);
            (interpolates, pacing.draw_divisor)
        };
        assert!(!run(1).0);
        assert_eq!(run(3), (true, 1));
    }

    #[test]
    fn a_crowded_room_left_for_a_lighter_one_interpolates_again() {
        // Yulgar with 20 players: 12 ms draws, once per frame; back in battleon, 5 ms draws
        // (drawn once per frame meanwhile, as they are while not interpolating): it comes back.
        let mut pacing = Pacing::new();
        pacing.set_refresh_rate(165_000);
        pacing.set_in_flight(3);
        for _ in 0..100 {
            pacing.record_frame(1.5);
            pacing.record_draw(12.0);
            pacing.record_pipeline(30.0);
        }
        let start = Instant::now();
        pacing.plan(start, 30.0);
        assert!(!pacing.plan(start + Duration::from_millis(600), 30.0));
        for _ in 0..100 {
            pacing.record_draw(5.0);
            pacing.record_pipeline(15.0);
        }
        pacing.plan(start + Duration::from_millis(700), 30.0);
        assert!(pacing.plan(start + Duration::from_millis(1800), 30.0));
        assert_eq!(pacing.draw_divisor, 2, "82.5 a second, as in battleon");
    }

    #[test]
    fn a_render_thread_has_a_budget_of_its_own() {
        // battleon at 165 Hz: the player's thread walks the stage in 2 ms, the render thread
        // records the picture in 6 ms. Charged to the player's thread, 6 ms with 30 frames a
        // second of 3 ms is every 3rd refresh; on its own budget, every 2nd.
        let mut pacing = Pacing::new();
        pacing.set_refresh_rate(165_000);
        for _ in 0..100 {
            pacing.record_frame(3.0);
            pacing.record_draw(2.0);
            pacing.record_thread(6.0);
        }
        let start = Instant::now();
        pacing.plan(start, 30.0);
        assert!(pacing.plan(start + Duration::from_millis(600), 30.0));
        assert_eq!(pacing.draw_divisor, 2);
    }

    #[test]
    fn a_way_too_long_for_interpolating_comes_back() {
        // At 60 Hz the step under every refresh is once per game frame.
        let mut pacing = Pacing::new();
        pacing.set_refresh_rate(60_000);
        for _ in 0..100 {
            pacing.record_draw(1.0);
            pacing.record_pipeline(20.0);
        }
        let start = Instant::now();
        pacing.plan(start, 30.0);
        assert!(!pacing.plan(start + Duration::from_millis(600), 30.0));
        // Still recorded once per frame meanwhile: short ways bring it back.
        for _ in 0..100 {
            pacing.record_pipeline(3.0);
        }
        pacing.plan(start + Duration::from_millis(700), 30.0);
        assert!(pacing.plan(start + Duration::from_millis(1800), 30.0));
        assert_eq!(pacing.draw_divisor, 1);
    }
}
