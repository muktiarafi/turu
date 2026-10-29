use ruffle_render::backend::{NO_STRETCH, ShapeHandle, Stretch};
use ruffle_render::tessellator::{EdgeMode, edge_mode};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

/// The scale ratio threshold beyond which a graphic or a drawing will be retessellated.
const RETESSELLATION_SCALE_THRESHOLD: f32 = 2.0;

/// The maximum number of retessellated shapes to cache per original shape.
const RETESSELLATION_CACHE_SIZE: usize = 12;

/// AQW build: how many tessellations a shape keeps: Ruffle's 4, or with Flash-style edges 12,
/// as each copy of a shape drawn squashed differently (a tree made of one branch, rotated and
/// stretched 10 ways) needs its own.
fn capacity() -> usize {
    if ruffle_render::tessellator::edge_aa() {
        RETESSELLATION_CACHE_SIZE
    } else {
        4
    }
}

#[derive(Clone, Debug)]
pub(crate) struct TessellationCache {
    /// (scale (AQW build: and stretch), handle, AQW build: when it was last drawn, see `clock`)
    entries: [Option<(Measure, ShapeHandle, u32)>; RETESSELLATION_CACHE_SIZE],
    len: usize,
    /// AQW build: the edge mode the entries were tessellated for (see `MadeFor`).
    mode: EdgeMode,
    /// AQW build: in how many different seconds a tessellation was missing only for the
    /// shape's stretch, and the last one (see `find_near_and_touch`).
    stretch_misses: u8,
    stretch_miss_at: u32,
}

/// AQW build: what a tessellation was made for. With Flash-style edges, a shape is tessellated
/// for the size it's drawn at and the quality's edge mode (its soft edges are part of it).
#[derive(Clone, Copy, Debug)]
pub(crate) struct MadeFor {
    measure: Measure,
    mode: EdgeMode,
}

impl MadeFor {
    pub(crate) fn now(measure: Measure) -> Self {
        Self {
            measure,
            mode: edge_mode(),
        }
    }

    /// Whether it can be drawn at `measure` now.
    pub(crate) fn fits(&self, measure: Measure) -> bool {
        self.mode == edge_mode() && measure.near(&self.measure)
    }

    /// How far `measure`'s scale is from the one it was made for (0: the same).
    pub(crate) fn deviation(&self, measure: Measure) -> f32 {
        (measure.scale / self.measure.scale - 1.0).abs()
    }
}

/// AQW build: the size a shape is drawn at, to tessellate it for: its scale and, with
/// Flash-style edges, how it's squashed (see `Stretch`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Measure {
    pub scale: f32,
    pub stretch: Stretch,
}

impl Measure {
    /// For a shape drawn with `matrix`. Ruffle's scale, from |a + c| and |b + d|, is fine for
    /// the flattening tolerance, but Flash-style edges (half a pixel wide at the scale
    /// tessellated at) need the size on screen: then the scale is the area's (exact for any
    /// rotation), and a shape squashed more than 1.3x (floor circles and rugs drawn flat) also
    /// gets its stretch, so its edges are half a pixel wide across the squashed direction too
    /// (a flat floor circle's thin line came out dashed). Less squash than that (characters'
    /// animations) counts as none, so animating it doesn't tessellate again.
    pub(crate) fn of(matrix: &ruffle_render::matrix::Matrix) -> Self {
        let scale_x = f32::abs(matrix.a + matrix.c);
        let scale_y = f32::abs(matrix.b + matrix.d);
        let ruffle = ((scale_x * scale_x + scale_y * scale_y) / 2.0).sqrt();
        if !ruffle_render::tessellator::edge_aa() {
            return Self {
                scale: ruffle,
                stretch: NO_STRETCH,
            };
        }
        let (a, b, c, d) = (matrix.a, matrix.b, matrix.c, matrix.d);
        let det = (a * d - b * c).abs();
        let scale = det.sqrt();
        if scale < ruffle * 0.05 {
            // Squashed nearly flat: nothing to see of its edges.
            return Self {
                scale: ruffle,
                stretch: NO_STRETCH,
            };
        }
        // The squash: the ratio of the largest to the smallest scale in any direction.
        let sum = a * a + b * b + c * c + d * d;
        let spread = (sum * sum - 4.0 * det * det).max(0.0).sqrt();
        let squash = ((sum + spread) / (sum - spread).max(f32::EPSILON)).sqrt();
        let detail = ruffle / scale;
        let stretch = if squash > 1.3 {
            [(a * a + b * b) / det, (a * c + b * d) / det, (c * c + d * d) / det, detail]
        } else {
            [1.0, 0.0, 1.0, detail]
        };
        Self { scale, stretch }
    }

    /// Whether a tessellation made for `cached` can be drawn at `self`.
    /// AQW build: a detailed shape (`records`) is tessellated for at least `MIN_SIZE` pixels
    /// across (its larger side, `bounds` in its own coordinates): drawn smaller (an effect
    /// growing from nothing, drawn at 0.01x), it's drawn with that tessellation. Made for its
    /// size, a 1-pixel tessellation of a 3,000-record shape with Flash-style edges (fades half
    /// a pixel wide, wider than the shape) took up to 275 ms and 90,000 vertices, again for
    /// every step it grew by (3.6 ms at full size, 11 ms at 9 px). Simple shapes are cheap at
    /// any size and keep their own tessellation: tiny glowing dots drawn see-through as thinner
    /// than a pixel looked closer to Flash than tessellated bigger.
    pub(crate) fn at_least_min_size(
        mut self,
        bounds: &swf::Rectangle<swf::Twips>,
        records: usize,
    ) -> Self {
        const MIN_SIZE: f32 = 8.0;
        const DETAILED: usize = 256;
        if records <= DETAILED {
            return self;
        }
        let largest = bounds.width().to_pixels().max(bounds.height().to_pixels()) as f32;
        if largest > 0.0 && largest * self.scale < MIN_SIZE {
            self.scale = MIN_SIZE / largest;
        }
        self
    }

    pub(crate) fn near(&self, cached: &Measure) -> bool {
        let [x0, y0, z0, d0] = self.stretch;
        let [x1, y1, z1, d1] = cached.stretch;
        near_scale(self.scale, cached.scale)
            && (x0 - x1).abs() + (y0 - y1).abs() + (z0 - z1).abs() < 0.3
            && (d0 / d1 - 1.0).abs() < 0.2
    }
}

thread_local! {
    /// AQW build: seconds since the player started, as of the last render (see `set_clock`).
    static CLOCK: Cell<u32> = const { Cell::new(0) };
    /// AQW build: every shared tessellation cache, for `evict_unused`.
    static REGISTRY: RefCell<Vec<Weak<RefCell<TessellationCache>>>> =
        const { RefCell::new(Vec::new()) };
    /// Registry length at which dead entries are next removed (see `new_shared`).
    static NEXT_PRUNE: Cell<usize> = const { Cell::new(4096) };
}

/// AQW build: whether a tessellation made at `cached` scale can be drawn at `target`. With
/// Flash-style edges its soft edges are half a pixel wide only at the scale it was made at
/// (2x off, they're a pixel or a quarter), so it's kept closer then.
pub(crate) fn near_scale(target: f32, cached: f32) -> bool {
    let threshold = if ruffle_render::tessellator::edge_aa() {
        std::f32::consts::SQRT_2
    } else {
        RETESSELLATION_SCALE_THRESHOLD
    };
    let ratio = f32::abs(target / cached);
    ratio <= threshold && ratio >= 1.0 / threshold
}

/// AQW build: sets the clock shapes are stamped with when drawn (called every render).
pub(crate) fn set_clock(secs: u32) {
    CLOCK.with(|c| c.set(secs));
}

fn clock() -> u32 {
    CLOCK.with(Cell::get)
}

/// AQW build: frees the tessellations (GPU geometry) of shapes not drawn in the last
/// `unused_secs` seconds; a shape drawn again later is tessellated again. AQW keeps every
/// gear SWF it ever loaded, so a long session (AFK in a busy town, players coming and going)
/// kept growing ~500 MB of shape geometry an hour, until a 4 GB card ran out (players arriving
/// drawn without their gear). Only used while the GPU's memory is nearly full. Returns how many
/// tessellations were freed.
pub fn evict_unused(unused_secs: u32) -> usize {
    let cutoff = clock().saturating_sub(unused_secs);
    REGISTRY.with(|registry| {
        let mut registry = registry.borrow_mut();
        let mut freed = 0;
        registry.retain(|cache| {
            let Some(cache) = cache.upgrade() else {
                return false;
            };
            // A cache in use right now (it can't be, between frames) is just skipped.
            if let Ok(mut cache) = cache.try_borrow_mut() {
                freed += cache.evict_older_than(cutoff);
            }
            true
        });
        freed
    })
}

/// A cache for tessellated shapes at different scales, using a simple LRU eviction policy.
/// (LRU index: 0, MRU index: len - 1)
///
/// The cache stores a fixed number of entries, and when it is full, the least recently
/// used entry is evicted to make room for new entries.
///
/// This is used to avoid retessellating shapes at similar scales multiple times, which can be expensive.
impl TessellationCache {
    /// Creates a new, empty tessellation cache.
    pub(crate) fn new() -> Self {
        Self {
            entries: std::array::from_fn(|_| None),
            len: 0,
            mode: edge_mode(),
            stretch_misses: 0,
            stretch_miss_at: u32::MAX,
        }
    }

    /// AQW build: a new, empty cache to share, registered for `evict_unused`.
    pub(crate) fn new_shared() -> Rc<RefCell<Self>> {
        let cache = Rc::new(RefCell::new(Self::new()));
        REGISTRY.with(|registry| {
            let mut registry = registry.borrow_mut();
            // Dead entries keep their (small) allocation; drop them now and then, so scripts
            // creating many `Shape`s over hours don't pile them up.
            if registry.len() >= NEXT_PRUNE.with(Cell::get) {
                registry.retain(|cache| cache.strong_count() > 0);
                NEXT_PRUNE.with(|next| next.set((registry.len() * 2).max(4096)));
            }
            registry.push(Rc::downgrade(&cache));
        });
        cache
    }

    /// AQW build: drops the entries when the edge mode has changed (the stage quality, with
    /// Flash-style edges).
    fn check_mode(&mut self) {
        let mode = edge_mode();
        if self.mode != mode {
            self.entries = std::array::from_fn(|_| None);
            self.len = 0;
            self.mode = mode;
        }
    }

    /// AQW build: drops the entries last drawn before `cutoff`; returns how many.
    fn evict_older_than(&mut self, cutoff: u32) -> usize {
        let mut kept = 0;
        for i in 0..self.len {
            let entry = self.entries[i].take();
            if let Some(entry) = entry
                && entry.2 >= cutoff
            {
                self.entries[kept] = Some(entry);
                kept += 1;
            }
        }
        let evicted = self.len - kept;
        self.len = kept;
        evicted
    }

    /// Finds the cached shape handle with the closest scale to the target scale.
    ///
    /// If the closest scale is NOT within the retessellation threshold,
    /// then `None` is returned, indicating that the shape should be retessellated at the target scale.
    ///
    /// AQW build: a shape whose stretch keeps changing (squashed and turning, like a magic
    /// circle spinning on a floor drawn flat) was tessellated again every frame; when it keeps
    /// missing only for the stretch, in 3 different seconds (copies of a shape drawn squashed
    /// differently miss only in the first frame), it's tessellated without one (`target` is
    /// changed to say so; tessellate for it on a miss).
    pub(crate) fn find_near_and_touch(&mut self, target: &mut Measure) -> Option<ShapeHandle> {
        let unstretched = |target: &mut Measure| {
            target.stretch = [1.0, 0.0, 1.0, target.stretch[3]];
        };
        if self.stretch_misses >= 3 {
            unstretched(target);
        }
        let found = self.find(*target);
        if found.is_none()
            && target.stretch[..3] != NO_STRETCH[..3]
            && self.entries[..self.len]
                .iter()
                .flatten()
                .any(|(cached, _, _)| near_scale(target.scale, cached.scale))
        {
            let now = clock();
            if self.stretch_miss_at != now {
                self.stretch_miss_at = now;
                self.stretch_misses += 1;
            }
            if self.stretch_misses >= 3 {
                unstretched(target);
                return self.find(*target);
            }
        }
        found
    }

    fn find(&mut self, target: Measure) -> Option<ShapeHandle> {
        self.check_mode();
        let mut best_index = None;
        let mut best_deviation = f32::INFINITY;

        for (index, entry) in self.entries[..self.len].iter().enumerate() {
            if let Some((cached, _, _)) = entry {
                let ratio = f32::abs(target.scale / cached.scale);

                // Check if the cached scale is within the retessellation threshold of the target scale.
                if target.near(cached) {
                    // Choose the cached entry with the smallest deviation from the target scale.
                    let deviation = f32::abs(ratio - 1.0);

                    if deviation < best_deviation {
                        best_deviation = deviation;
                        best_index = Some(index);
                    }
                }
            }
        }

        // If we found a suitable cached entry, move it to the most recently used position.
        best_index.map(|index| self.touch_entry(index))
    }

    /// Inserts a new cached shape handle with the given scale.
    ///
    /// If the cache is full, the least recently used entry will be evicted to make room for the new entry.
    /// We assume that the caller has already checked that the new entry is not too similar to existing entries.
    pub(crate) fn insert(&mut self, measure: Measure, handle: ShapeHandle) {
        self.check_mode();
        if self.len < capacity() {
            // If the cache is not full, simply add the new entry at the end.
            self.entries[self.len] = Some((measure, handle, clock()));
            self.len += 1;
            return;
        }

        // If the cache is full, evict the least recently used entry.
        for i in 1..self.len {
            self.entries[i - 1] = self.entries[i].take();
        }

        self.entries[self.len - 1] = Some((measure, handle, clock()));
    }

    /// AQW build: swaps the entry holding `old` (compared by identity) for `new`, keeping its
    /// scale and position; with `None` the entry is removed. Used to fill in a deferred
    /// tessellation (see `graphic::deferred_shapes`).
    pub(crate) fn replace(&mut self, old: &ShapeHandle, new: Option<ShapeHandle>) {
        let Some(index) = self.entries[..self.len].iter().position(|entry| {
            entry
                .as_ref()
                .is_some_and(|(_, handle, _)| std::sync::Arc::ptr_eq(&handle.0, &old.0))
        }) else {
            return;
        };
        match new {
            Some(new) => {
                if let Some((_, handle, _)) = &mut self.entries[index] {
                    *handle = new;
                }
            }
            None => {
                for i in index + 1..self.len {
                    self.entries[i - 1] = self.entries[i].take();
                }
                self.entries[self.len - 1] = None;
                self.len -= 1;
            }
        }
    }

    /// Returns the number of cached entries currently stored in the cache.
    pub(crate) fn len(&self) -> usize {
        self.len
    }

    /// Moves the entry at the given index to the most recently used position and returns its shape handle.
    fn touch_entry(&mut self, index: usize) -> ShapeHandle {
        if let Some(entry) = &mut self.entries[index] {
            entry.2 = clock();
        }
        if index == self.len - 1 {
            // The entry is already the most recently used, so we can return its handle directly.
            return self.entries[index]
                .as_ref()
                .expect("tessellation cache entry exists")
                .1
                .clone();
        }

        let entry = self.entries[index].take().expect("entry exists");

        for i in index + 1..self.len {
            self.entries[i - 1] = self.entries[i].take();
        }

        // Move touched entry to MRU position (end)
        self.entries[self.len - 1] = Some(entry.clone());
        entry.1
    }
}
