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
    pub fn new() -> Result<Self, Error> {
        let mut backend = Self {
            stream: None,
            mixer: AudioMixer::new(2, 44100),
            paused: true,
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
