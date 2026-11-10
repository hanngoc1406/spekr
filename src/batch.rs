use crate::audio::{decode_audio, ChannelSelection};
use crate::palette::PaletteType;
use crate::pipeline::{SpectrogramConfig, SpectrogramData};
use crate::render::render_spectrogram_image;
use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

pub const SUPPORTED_EXTENSIONS: &[&str] = &[
    "flac", "mp3", "wav", "m4a", "aac", "ogg", "alac", "aiff", "aif", "wma", "opus",
];

pub fn is_audio_file(path: &Path) -> bool {
    if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
        let ext_lower = ext.to_lowercase();
        SUPPORTED_EXTENSIONS.contains(&ext_lower.as_str())
    } else {
        false
    }
}

pub fn scan_directory(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                files.extend(scan_directory(&path));
            } else if is_audio_file(&path) {
                files.push(path);
            }
        }
    }
    files.sort();
    files
}

pub struct BatchResult {
    pub total: usize,
    pub succeeded: usize,
    pub failed: usize,
}

pub fn batch_process_files(
    files: &[PathBuf],
    output_dir: &Path,
    palette: PaletteType,
    width: u32,
    height: u32,
    progress_cb: impl Fn(usize, usize) + Sync + Send,
) -> BatchResult {
    let _ = std::fs::create_dir_all(output_dir);
    let total = files.len();
    let completed = Arc::new(AtomicUsize::new(0));
    let succeeded = Arc::new(AtomicUsize::new(0));
    let failed = Arc::new(AtomicUsize::new(0));

    files.par_iter().for_each(|file_path| {
        let file_stem = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(crate::fonts::to_nfc)
            .unwrap_or_else(|| "track".to_string());
        let out_png = output_dir.join(format!("{}_spectrogram.png", file_stem));

        let res = decode_audio(file_path, ChannelSelection::Mix, |_| {});
        match res {
            Ok((info, samples)) => {
                let config = SpectrogramConfig {
                    fft_size: 2048,
                    window_fn: crate::fft::WindowFunction::Hann,
                    num_columns: width.max(100) as usize,
                    min_db: -120.0,
                    max_db: 0.0,
                };
                let data = SpectrogramData::compute(info, &samples, config, |_| {});
                let img = render_spectrogram_image(&data, palette, -120.0, 0.0, width, height);
                if img.save(&out_png).is_ok() {
                    succeeded.fetch_add(1, Ordering::Relaxed);
                } else {
                    failed.fetch_add(1, Ordering::Relaxed);
                }
            }
            Err(_) => {
                failed.fetch_add(1, Ordering::Relaxed);
            }
        }

        let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
        progress_cb(done, total);
    });

    BatchResult {
        total,
        succeeded: succeeded.load(Ordering::Relaxed),
        failed: failed.load(Ordering::Relaxed),
    }
}
