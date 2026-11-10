use crate::audio::AudioInfo;
use crate::fft::{FftProcessor, WindowFunction};
use rayon::prelude::*;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SpectrogramConfig {
    pub fft_size: usize,
    pub window_fn: WindowFunction,
    pub num_columns: usize,
    pub min_db: f32,
    pub max_db: f32,
}

impl Default for SpectrogramConfig {
    fn default() -> Self {
        Self {
            fft_size: 2048,
            window_fn: WindowFunction::Hann,
            num_columns: 1000,
            min_db: -120.0,
            max_db: 0.0,
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct SpectrogramData {
    pub info: AudioInfo,
    pub config: SpectrogramConfig,
    /// Matrix of shape [num_columns][num_bins] where each element is magnitude in dB
    pub matrix: Vec<Vec<f32>>,
    pub num_bins: usize,
    pub nyquist_freq: f32,
}

impl SpectrogramData {
    pub fn compute(
        info: AudioInfo,
        samples: &[f32],
        config: SpectrogramConfig,
        progress_cb: impl Fn(f32) + Sync + Send,
    ) -> Self {
        let num_bins = config.fft_size / 2;
        let nyquist_freq = info.sample_rate as f32 / 2.0;
        let total_samples = samples.len();
        let num_cols = config.num_columns.max(10);

        if total_samples == 0 {
            return Self {
                info,
                config: config.clone(),
                matrix: vec![vec![config.min_db; num_bins]; num_cols],
                num_bins,
                nyquist_freq,
            };
        }

        let fft_size = config.fft_size;
        let window_fn = config.window_fn;

        // Step size between consecutive columns
        let step = if total_samples > fft_size && num_cols > 1 {
            (total_samples - fft_size) as f64 / (num_cols - 1) as f64
        } else {
            0.0
        };

        let done_counter = std::sync::atomic::AtomicUsize::new(0);

        // Compute columns in parallel using Rayon
        let matrix: Vec<Vec<f32>> = (0..num_cols)
            .into_par_iter()
            .map_init(
                || FftProcessor::new(fft_size, window_fn),
                |processor, col_idx| {
                    let mut col_db = vec![0.0f32; num_bins];
                    let start_pos = (col_idx as f64 * step).round() as usize;

                    let slice = if start_pos + fft_size <= total_samples {
                        &samples[start_pos..start_pos + fft_size]
                    } else if start_pos < total_samples {
                        &samples[start_pos..]
                    } else {
                        &[]
                    };

                    processor.process(slice, &mut col_db);

                    let finished = done_counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                    if finished % 40 == 0 || finished == num_cols {
                        progress_cb((finished as f32 / num_cols as f32).clamp(0.0, 1.0));
                    }

                    col_db
                },
            )
            .collect();

        progress_cb(1.0);

        Self {
            info,
            config,
            matrix,
            num_bins,
            nyquist_freq,
        }
    }

    /// Sample dB value at specific time (seconds) and frequency (Hz)
    pub fn get_value_at(&self, time_sec: f64, freq_hz: f32) -> Option<f32> {
        if self.matrix.is_empty() || self.info.duration_seconds <= 0.0 {
            return None;
        }

        let col_f = (time_sec / self.info.duration_seconds) * (self.matrix.len() as f64);
        let col = col_f.clamp(0.0, (self.matrix.len() - 1) as f64) as usize;

        let bin_f = (freq_hz / self.nyquist_freq) * (self.num_bins as f32);
        let bin = bin_f.clamp(0.0, (self.num_bins - 1) as f32) as usize;

        self.matrix.get(col).and_then(|c| c.get(bin).copied())
    }
}
