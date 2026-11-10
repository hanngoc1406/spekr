use rodio::buffer::SamplesBuffer;
use rodio::{OutputStream, OutputStreamHandle, Sink};
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub struct AudioPlayer {
    _stream: Option<OutputStream>,
    stream_handle: Option<OutputStreamHandle>,
    sink: Option<Sink>,
    samples: Option<Arc<Vec<f32>>>,
    channels: u16,
    sample_rate: u32,
    duration_secs: f64,
    start_offset: f64,
    play_start_instant: Option<Instant>,
    is_paused: bool,
}

impl AudioPlayer {
    pub fn new() -> Arc<Mutex<Self>> {
        let (stream, stream_handle) = match OutputStream::try_default() {
            Ok((s, h)) => (Some(s), Some(h)),
            Err(e) => {
                log::warn!("Could not initialize audio output stream: {e}");
                (None, None)
            }
        };

        Arc::new(Mutex::new(Self {
            _stream: stream,
            stream_handle,
            sink: None,
            samples: None,
            channels: 2,
            sample_rate: 44100,
            duration_secs: 0.0,
            start_offset: 0.0,
            play_start_instant: None,
            is_paused: false,
        }))
    }

    pub fn load_track(&mut self, samples: Arc<Vec<f32>>, channels: usize, sample_rate: u32, duration: f64) {
        self.stop();
        // Determine the actual channel count from sample buffer length vs duration
        let actual_channels = if duration > 0.0 && sample_rate > 0 {
            let expected_frames = (duration * sample_rate as f64).round() as usize;
            if expected_frames > 0 {
                let ratio = (samples.len() as f64 / expected_frames as f64).round() as usize;
                ratio.clamp(1, channels.max(1))
            } else {
                channels.max(1)
            }
        } else {
            channels.max(1)
        };

        self.samples = Some(samples);
        self.channels = actual_channels as u16;
        self.sample_rate = sample_rate.max(8000);
        self.duration_secs = duration;
        self.start_offset = 0.0;
        self.play_start_instant = None;
        self.is_paused = false;
    }

    pub fn play(&mut self) {
        if self.is_paused {
            if let Some(sink) = &self.sink {
                sink.play();
                self.is_paused = false;
                self.play_start_instant = Some(Instant::now());
                return;
            }
        }
        self.play_from(self.start_offset);
    }

    pub fn play_from(&mut self, offset_secs: f64) {
        let handle = match &self.stream_handle {
            Some(h) => h,
            None => return,
        };

        let samples = match &self.samples {
            Some(s) => s,
            None => return,
        };

        if let Some(sink) = self.sink.take() {
            sink.stop();
        }

        let new_sink = match Sink::try_new(handle) {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to create audio sink: {e}");
                return;
            }
        };

        let ch = self.channels as usize;
        let start_frame = ((offset_secs * self.sample_rate as f64).round() as usize).min(samples.len() / ch);
        let start_sample_idx = (start_frame * ch).min(samples.len());

        let remaining_samples = samples[start_sample_idx..].to_vec();
        if !remaining_samples.is_empty() {
            let buffer = SamplesBuffer::new(self.channels, self.sample_rate, remaining_samples);
            new_sink.append(buffer);
            new_sink.play();
            self.sink = Some(new_sink);
            self.start_offset = offset_secs;
            self.play_start_instant = Some(Instant::now());
            self.is_paused = false;
        }
    }

    pub fn pause(&mut self) {
        if let Some(sink) = &self.sink {
            sink.pause();
            self.is_paused = true;
            if let Some(instant) = self.play_start_instant.take() {
                self.start_offset += instant.elapsed().as_secs_f64();
            }
        }
    }

    pub fn toggle_play_pause(&mut self) {
        if self.is_playing() {
            self.pause();
        } else {
            self.play();
        }
    }

    pub fn stop(&mut self) {
        if let Some(sink) = self.sink.take() {
            sink.stop();
        }
        self.play_start_instant = None;
        self.start_offset = 0.0;
        self.is_paused = false;
    }

    pub fn is_playing(&self) -> bool {
        if self.is_paused {
            return false;
        }
        if let Some(sink) = &self.sink {
            !sink.is_paused() && !sink.empty()
        } else {
            false
        }
    }

    pub fn current_time(&self) -> f64 {
        if self.is_playing() {
            let elapsed = self
                .play_start_instant
                .map(|i| i.elapsed().as_secs_f64())
                .unwrap_or(0.0);
            (self.start_offset + elapsed).min(self.duration_secs)
        } else {
            self.start_offset.min(self.duration_secs)
        }
    }

    pub fn seek(&mut self, time_secs: f64) {
        let target = time_secs.clamp(0.0, self.duration_secs);
        if self.is_playing() {
            self.play_from(target);
        } else {
            self.start_offset = target;
        }
    }

    pub fn set_volume(&mut self, volume: f32) {
        if let Some(sink) = &self.sink {
            sink.set_volume(volume.clamp(0.0, 1.5));
        }
    }
}
