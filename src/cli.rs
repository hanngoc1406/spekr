use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "spekr",
    author = "Duc Phan & Spekr Contributors",
    version,
    about = "A fast, modern acoustic spectrum analyser written in Rust (successor to Spek)",
    long_about = "Spekr is a cross-platform acoustic spectrum analyser. It can run in GUI mode or headless CLI mode to generate PNG spectrograms."
)]
pub struct CliArgs {
    /// Input audio file path
    #[arg(value_name = "AUDIO_FILE")]
    pub input: Option<PathBuf>,

    /// Headless mode: export spectrogram image to this path (e.g. output.png)
    #[arg(short, long, value_name = "IMAGE_FILE")]
    pub output: Option<PathBuf>,

    /// Image width in pixels (CLI export mode)
    #[arg(long, default_value_t = 1200)]
    pub width: u32,

    /// Image height in pixels (CLI export mode)
    #[arg(long, default_value_t = 600)]
    pub height: u32,

    /// FFT window size (1024, 2048, 4096, 8192)
    #[arg(long, default_value_t = 2048)]
    pub fft: usize,

    /// Window function: hann, hamming, blackman-harris, flattop, rectangular
    #[arg(long, default_value = "hann")]
    pub window: String,

    /// Color palette: sox, classic, magma, viridis, inferno, turbo, grayscale
    #[arg(long, default_value = "sox")]
    pub palette: String,

    /// Minimum dB dynamic range limit
    #[arg(long, default_value_t = -120.0)]
    pub min_db: f32,

    /// Maximum dB dynamic range limit
    #[arg(long, default_value_t = 0.0)]
    pub max_db: f32,

    /// Channel: mix, 1 (left), 2 (right)
    #[arg(long, default_value = "mix")]
    pub channel: String,
}
