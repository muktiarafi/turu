//! Turu: Notify Drop's beep on the desktop (`Platform::beep`): a stream of its own on the
//! system's default output, as .NET's `Console.Beep()` sounds apart from the game. Ruffle's
//! audio (and its mute) isn't involved.

use crate::platform::{BEEP_MS, beep_sample};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, SampleFormat, SizedSample, StreamConfig};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Plays one beep and returns once it has played (or failed, logged).
pub fn beep() {
    if let Err(e) = play() {
        tracing::warn!("Couldn't play the beep: {e}");
    }
}

fn play() -> Result<(), Box<dyn std::error::Error>> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or("no sound output")?;
    let supported = device.default_output_config()?;
    let format = supported.sample_format();
    let config = supported.config();
    let played = Arc::new(AtomicU64::new(0));
    let stream = match format {
        SampleFormat::F32 => stream::<f32>(&device, &config, played.clone())?,
        SampleFormat::I16 => stream::<i16>(&device, &config, played.clone())?,
        SampleFormat::U16 => stream::<u16>(&device, &config, played.clone())?,
        SampleFormat::I32 => stream::<i32>(&device, &config, played.clone())?,
        SampleFormat::F64 => stream::<f64>(&device, &config, played.clone())?,
        SampleFormat::I8 => stream::<i8>(&device, &config, played.clone())?,
        SampleFormat::U8 => stream::<u8>(&device, &config, played.clone())?,
        other => return Err(format!("unsupported sample format {other}").into()),
    };
    stream.play()?;
    // Until the tone's frames went out, plus a little for the device's buffer; the stream stops
    // when dropped.
    let length = u64::from(config.sample_rate.0) * BEEP_MS / 1000;
    let deadline = Instant::now() + Duration::from_millis(BEEP_MS + 500);
    while played.load(Ordering::Relaxed) < length && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    std::thread::sleep(Duration::from_millis(30));
    Ok(())
}

fn stream<T: SizedSample + FromSample<f32>>(
    device: &cpal::Device,
    config: &StreamConfig,
    played: Arc<AtomicU64>,
) -> Result<cpal::Stream, cpal::BuildStreamError> {
    let channels = usize::from(config.channels).max(1);
    let rate = config.sample_rate.0;
    device.build_output_stream(
        config,
        move |data: &mut [T], _| {
            let mut frame = played.load(Ordering::Relaxed);
            for samples in data.chunks_mut(channels) {
                let value = T::from_sample(beep_sample(frame, rate));
                samples.fill(value);
                frame += 1;
            }
            played.store(frame, Ordering::Relaxed);
        },
        |e| tracing::warn!("Beep stream: {e}"),
        None,
    )
}
