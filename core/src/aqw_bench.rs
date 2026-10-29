//! AQW build: an offline load benchmark (a test, run on demand).
//!
//! Loads real AQW files from `AQW_CORPUS` (a folder, searched for .swf files) into one player
//! the way the game loads gear: each through AS3's `Loader.loadBytes`, the loader added to the
//! stage so the loaded movie runs its first frames, `AQW_REPEAT` times over (default 3, as
//! several players wear the same item). Then reports the heap the loads left (every Rust
//! allocation is counted), the time they took and the shapes shared. The same files in the
//! same order every run, without a login or a server: before/after comparisons are exact.
//!
//! `AQW_CORPUS=<folder> cargo test --release --locked -p ruffle_core aqw_load_bench -- --ignored --nocapture`

use crate::avm2::bytearray::ByteArrayStorage;
use crate::avm2::object::ByteArrayObject;
use crate::avm2::{Activation, FunctionArgs, Value};
use crate::display_object::TDisplayObject;
use crate::player::PlayerBuilder;
use crate::string::AvmString;
use crate::tag_utils::SwfMovie;
use ruffle_common::duration::FloatDuration;
use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicI64, Ordering};
use std::time::Instant;

/// Bytes allocated and not freed, by the whole test binary.
static HEAP: AtomicI64 = AtomicI64::new(0);

struct Counting;

// SAFETY: forwards to the system allocator, only counting.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc(layout) };
        if !ptr.is_null() {
            HEAP.fetch_add(layout.size() as i64, Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        let ptr = unsafe { System.alloc_zeroed(layout) };
        if !ptr.is_null() {
            HEAP.fetch_add(layout.size() as i64, Ordering::Relaxed);
        }
        ptr
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) };
        HEAP.fetch_sub(layout.size() as i64, Ordering::Relaxed);
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let new = unsafe { System.realloc(ptr, layout, new_size) };
        if !new.is_null() {
            HEAP.fetch_add(new_size as i64 - layout.size() as i64, Ordering::Relaxed);
        }
        new
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

const MB: f64 = 1024.0 * 1024.0;

fn heap() -> i64 {
    HEAP.load(Ordering::Relaxed)
}

/// Every .swf file under `dir`, sorted (the same order every run).
fn swf_files(dir: &std::path::Path, files: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            swf_files(&path, files);
        } else if path.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("swf")) {
            files.push(path);
        }
    }
}

/// A one-frame AS3 movie to be the root (the game's own client is the root in AQW).
fn root_movie() -> SwfMovie {
    let header = swf::Header {
        compression: swf::Compression::None,
        version: 32,
        stage_size: swf::Rectangle {
            x_min: swf::Twips::ZERO,
            x_max: swf::Twips::from_pixels(960.0),
            y_min: swf::Twips::ZERO,
            y_max: swf::Twips::from_pixels(550.0),
        },
        frame_rate: swf::Fixed8::from_f32(24.0),
        num_frames: 1,
    };
    let tags = [
        swf::Tag::FileAttributes(swf::FileAttributes::IS_ACTION_SCRIPT_3),
        swf::Tag::ShowFrame,
    ];
    let mut bytes = Vec::new();
    swf::write_swf(&header, &tags, &mut bytes).expect("root movie");
    SwfMovie::from_data(&bytes, "file:///aqw_bench.swf".to_string(), None, None).expect("root")
}

#[test]
#[ignore]
fn aqw_load_bench() {
    let dir = std::env::var("AQW_CORPUS").expect("AQW_CORPUS: a folder of AQW .swf files");
    let repeat: usize = std::env::var("AQW_REPEAT")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(3);
    let mut files = Vec::new();
    swf_files(std::path::Path::new(&dir), &mut files);
    files.sort();
    let data: Vec<(String, Vec<u8>)> = files
        .iter()
        .filter_map(|path| Some((path.display().to_string(), std::fs::read(path).ok()?)))
        .collect();
    let file_bytes: usize = data.iter().map(|(_, bytes)| bytes.len()).sum();

    let start_heap = heap();
    let player = PlayerBuilder::new().with_movie(root_movie()).build();
    let mut player = player.lock().expect("player");
    player.set_is_playing(true);
    let frame = FloatDuration::from_millis(1000.0 / 24.0);
    for _ in 0..3 {
        player.tick(frame);
    }
    let base_heap = heap();

    let started = Instant::now();
    let (mut loaded, mut failed) = (0usize, 0usize);
    for round in 0..repeat {
        for (name, bytes) in &data {
            let ok = player.mutate_with_update_context(|context| {
                // As the game client's code (its domain), which is what calls `Loader` in AQW.
                let domain = context.avm2.stage_domain();
                let mut activation = Activation::from_domain(context, domain);
                let load = |activation: &mut Activation<'_, '_>| -> Result<(), String> {
                    let loader_class = activation.avm2().classes().loader;
                    let loader = loader_class
                        .construct(activation, &[])
                        .map_err(|e| format!("{e:?}"))?;
                    let storage = ByteArrayStorage::from_vec(activation.context, bytes.clone());
                    let byte_array = ByteArrayObject::from_storage(activation.context, storage);
                    let load_bytes = AvmString::new_utf8(activation.gc(), "loadBytes");
                    loader
                        .call_public_property(
                            load_bytes,
                            FunctionArgs::from_slice(&[byte_array.into()]),
                            activation,
                        )
                        .map_err(|e| format!("{e:?}"))?;
                    let stage = activation
                        .context
                        .stage
                        .object2()
                        .ok_or("no AS3 stage")?;
                    let add_child = AvmString::new_utf8(activation.gc(), "addChild");
                    Value::from(stage)
                        .call_public_property(add_child, FunctionArgs::from_slice(&[loader]), activation)
                        .map_err(|e| format!("{e:?}"))?;
                    Ok(())
                };
                match load(&mut activation) {
                    Ok(()) => true,
                    Err(e) => {
                        if round == 0 {
                            eprintln!("load failed: {name}: {e}");
                        }
                        false
                    }
                }
            });
            if ok {
                loaded += 1;
            } else {
                failed += 1;
            }
            // A few frames for each, as the game runs while gear streams in: preloading,
            // the class definitions, the first frames.
            for _ in 0..2 {
                player.tick(frame);
            }
        }
    }
    // Settle: the last loads finish, and the garbage collector catches up.
    for _ in 0..48 {
        player.tick(frame);
    }
    let elapsed = started.elapsed();
    let end_heap = heap();
    let (parsed, shared) = crate::display_object::graphic::shared_shapes::counts();
    let movies = player.mutate_with_update_context(|context| context.library.loaded_movie_count());

    println!("AQW load benchmark: {} files ({:.1} MB), {repeat} rounds", data.len(), file_bytes as f64 / MB);
    println!("  loads: {loaded} ok, {failed} failed, {:.1} s ({:.2} ms per load)", elapsed.as_secs_f64(), elapsed.as_secs_f64() * 1000.0 / loaded.max(1) as f64);
    println!("  player alone: {:.1} MB", (base_heap - start_heap) as f64 / MB);
    println!(
        "  heap from the loads: {:.1} MB ({:.2} MB per load), {movies} movies alive",
        (end_heap - base_heap) as f64 / MB,
        (end_heap - base_heap) as f64 / MB / loaded.max(1) as f64
    );
    println!("  shape tags: parsed {parsed}, shared {shared}");
}
