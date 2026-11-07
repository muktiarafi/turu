//! Sound through AAudio, from ruffle-android's backend (MIT/Apache,
//! https://github.com/ruffle-rs/ruffle-android `src/audio.rs`): Ruffle's mixer fills an AAudio
//! stream from its callback. Failures are logged instead of panicking: a missing sound
//! device must not take the bot down.

use ndk::audio::{AudioDirection, AudioFormat, AudioStream, AudioStreamBuilder, AudioStreamState};
use ruffle_core::backend::audio::{
    AudioBackend, AudioMixer, DecodeError, RegisterError, SoundHandle, SoundInstanceHandle, SoundStreamInfo,
    SoundTransform, swf,
};
use ruffle_core::impl_audio_mixer_backend;

pub struct AAudioAudioBackend {
    stream: Option<AudioStream>,
    mixer: AudioMixer,
    paused: bool,
}

type Error = Box<dyn std::error::Error>;

impl AAudioAudioBackend {
    /// Opens the stream already started, as the desktop's cpal backend does: an autoplaying
    /// player (`PlayerBuilder::with_autoplay`) starts out playing without calling `play`, so a
    /// stream left paused here would never start.
    pub fn new() -> Result<Self, Error> {
        let mut backend = Self {
            stream: None,
            mixer: AudioMixer::new(2, 44100),
            paused: false,
        };
        backend.recreate_stream()?;
        Ok(backend)
    }

    fn recreate_stream(&mut self) -> Result<(), Error> {
        let proxy = self.mixer.proxy();
        let stream = AudioStreamBuilder::new()?
            .direction(AudioDirection::Output)
            .format(AudioFormat::PCM_Float)
            .channel_count(2)
            .sample_rate(44100)
            .performance_mode(ndk::audio::AudioPerformanceMode::LowLatency)
            .data_callback(Box::new(move |_stream, data, frames| {
                // SAFETY: AAudio hands a buffer of `frames` stereo f32 frames.
                let samples = unsafe { std::slice::from_raw_parts_mut(data.cast::<f32>(), frames as usize * 2) };
                proxy.mix(samples);
                ndk::audio::AudioCallbackResult::Continue
            }))
            .open_stream()?;
        if !self.paused {
            stream.request_start()?;
        }
        self.stream = Some(stream);
        Ok(())
    }

    /// Opens a new stream when the device went away (headphones unplugged, ...).
    pub fn recreate_stream_if_needed(&mut self) {
        if self
            .stream
            .as_ref()
            .is_some_and(|s| s.state() == AudioStreamState::Disconnected)
            && let Err(e) = self.recreate_stream()
        {
            tracing::warn!("Couldn't reopen the sound stream: {e}");
        }
    }
}

impl AudioBackend for AAudioAudioBackend {
    impl_audio_mixer_backend!(mixer);

    fn play(&mut self) {
        if let Some(Err(e)) = self.stream.as_mut().map(|s| s.request_start()) {
            tracing::warn!("Couldn't start sound: {e}");
        }
        self.paused = false;
    }

    fn pause(&mut self) {
        if let Some(Err(e)) = self.stream.as_mut().map(|s| s.request_pause()) {
            tracing::warn!("Couldn't pause sound: {e}");
        }
        self.paused = true;
    }
}

/// Notify Drop's beep (`Platform::beep`): an AAudio stream of its own, apart from the game's, so
/// it sounds while the game is muted. Returns once it has played (or failed, logged).
pub fn beep() {
    if let Err(e) = play_beep() {
        tracing::warn!("Couldn't play the beep: {e}");
    }
}

fn play_beep() -> Result<(), Error> {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{Duration, Instant};
    use turu_frontend::platform::{BEEP_MS, beep_sample};

    const RATE: u32 = 44100;
    let played = Arc::new(AtomicU64::new(0));
    let counter = played.clone();
    let stream = AudioStreamBuilder::new()?
        .direction(AudioDirection::Output)
        .format(AudioFormat::PCM_Float)
        .channel_count(1)
        .sample_rate(RATE as i32)
        .data_callback(Box::new(move |_stream, data, frames| {
            // SAFETY: AAudio hands a buffer of `frames` mono f32 frames.
            let samples = unsafe { std::slice::from_raw_parts_mut(data.cast::<f32>(), frames as usize) };
            let mut frame = counter.load(Ordering::Relaxed);
            for sample in samples {
                *sample = beep_sample(frame, RATE);
                frame += 1;
            }
            counter.store(frame, Ordering::Relaxed);
            ndk::audio::AudioCallbackResult::Continue
        }))
        .open_stream()?;
    stream.request_start()?;
    let length = u64::from(RATE) * BEEP_MS / 1000;
    let deadline = Instant::now() + Duration::from_millis(BEEP_MS + 500);
    while played.load(Ordering::Relaxed) < length && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    std::thread::sleep(Duration::from_millis(30));
    let _ = stream.request_stop();
    Ok(())
}
