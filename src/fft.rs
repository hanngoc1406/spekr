use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};
use std::f32::consts::PI;
use std::sync::Arc;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(dead_code)]
pub enum WindowFunction {
    #[default]
    Hann,
    Hamming,
    BlackmanHarris,
    FlatTop,
    Rectangular,
}

#[allow(dead_code)]
impl WindowFunction {
    pub const ALL: &'static [WindowFunction] = &[
        WindowFunction::Hann,
        WindowFunction::Hamming,
        WindowFunction::BlackmanHarris,
        WindowFunction::FlatTop,
        WindowFunction::Rectangular,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            WindowFunction::Hann => "Hann",
            WindowFunction::Hamming => "Hamming",
            WindowFunction::BlackmanHarris => "Blackman-Harris",
            WindowFunction::FlatTop => "Flat Top",
            WindowFunction::Rectangular => "Rectangular",
        }
    }

    pub fn generate_window(&self, size: usize) -> Vec<f32> {
        let mut window = Vec::with_capacity(size);
        if size <= 1 {
            return vec![1.0; size];
        }

        let n_denom = (size - 1) as f32;
        let factor = 2.0 * PI / n_denom;

        for i in 0..size {
            let i_f = i as f32;
            let val = match self {
                WindowFunction::Hann => 0.5 * (1.0 - (factor * i_f).cos()),
                WindowFunction::Hamming => 0.53836 - 0.46164 * (factor * i_f).cos(),
                WindowFunction::BlackmanHarris => {
                    0.35875 - 0.48829 * (factor * i_f).cos()
                        + 0.14128 * (2.0 * factor * i_f).cos()
                        - 0.01168 * (3.0 * factor * i_f).cos()
                }
                WindowFunction::FlatTop => {
                    0.21557895 - 0.41663158 * (factor * i_f).cos()
                        + 0.277263158 * (2.0 * factor * i_f).cos()
                        - 0.083578947 * (3.0 * factor * i_f).cos()
                        + 0.006947368 * (4.0 * factor * i_f).cos()
                }
                WindowFunction::Rectangular => 1.0,
            };
            window.push(val);
        }
        window
    }
}

pub struct FftProcessor {
    fft: Arc<dyn Fft<f32>>,
    window: Vec<f32>,
    scratch: Vec<Complex<f32>>,
    fft_size: usize,
    norm_factor: f32,
}

impl FftProcessor {
    pub fn new(fft_size: usize, window_fn: WindowFunction) -> Self {
        let mut planner = FftPlanner::new();
        let fft = planner.plan_fft_forward(fft_size);
        let window = window_fn.generate_window(fft_size);
        let scratch = vec![Complex::default(); fft_size];
        let norm_factor = 1.0 / ((fft_size * fft_size) as f32);

        Self {
            fft,
            window,
            scratch,
            fft_size,
            norm_factor,
        }
    }

    #[allow(dead_code)]
    pub fn fft_size(&self) -> usize {
        self.fft_size
    }

    pub fn num_bins(&self) -> usize {
        self.fft_size / 2
    }

    /// Process input slice of length fft_size into output slice of length fft_size / 2 (in dB)
    pub fn process(&mut self, input: &[f32], output_db: &mut [f32]) {
        let in_len = input.len().min(self.fft_size);

        for i in 0..in_len {
            self.scratch[i] = Complex::new(input[i] * self.window[i], 0.0);
        }
        for i in in_len..self.fft_size {
            self.scratch[i] = Complex::default();
        }

        self.fft.process(&mut self.scratch);

        let num_bins = self.num_bins().min(output_db.len());
        for (i, bin) in output_db.iter_mut().take(num_bins).enumerate() {
            let re = self.scratch[i].re;
            let im = self.scratch[i].im;
            let power = (re * re + im * im) * self.norm_factor;
            // Floor power to avoid log10(0)
            let power = power.max(1e-14);
            *bin = 10.0 * power.log10();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_generation_symmetry() {
        let size = 1024;
        for &w in WindowFunction::ALL {
            let win = w.generate_window(size);
            assert_eq!(win.len(), size);
            // Window functions should be symmetric
            for i in 0..size / 2 {
                let diff = (win[i] - win[size - 1 - i]).abs();
                assert!(diff < 1e-4, "Asymmetry in window {:?} at index {}", w, i);
            }
        }
    }

    #[test]
    fn test_fft_pure_sine_wave() {
        let fft_size = 1024;
        let mut processor = FftProcessor::new(fft_size, WindowFunction::Hann);
        let sample_rate = 44100.0f32;
        let target_freq = 4410.0f32; // Exactly at bin 4410 / (44100 / 1024) = bin 102.4 ~ 102

        let mut input = vec![0.0f32; fft_size];
        for (i, x) in input.iter_mut().enumerate() {
            let t = i as f32 / sample_rate;
            *x = (2.0 * PI * target_freq * t).sin();
        }

        let mut output_db = vec![0.0f32; processor.num_bins()];
        processor.process(&input, &mut output_db);

        // Find peak bin
        let mut peak_bin = 0;
        let mut max_db = f32::NEG_INFINITY;
        for (bin, &db) in output_db.iter().enumerate() {
            if db > max_db {
                max_db = db;
                peak_bin = bin;
            }
        }

        // Peak bin should be around 102
        assert!(
            (peak_bin as isize - 102).abs() <= 2,
            "Peak expected around bin 102, got bin {}",
            peak_bin
        );
        assert!(max_db > -20.0, "Sine peak should be prominent, got {:.1} dB", max_db);
    }
}
