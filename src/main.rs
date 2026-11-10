mod app;
mod audio;
mod batch;
mod cli;
mod fft;
mod fonts;
mod palette;
mod pipeline;
mod playback;
mod render;

use anyhow::{anyhow, Result};
use app::SpekrApp;
use audio::{decode_audio, ChannelSelection};
use batch::{batch_process_files, scan_directory};
use clap::Parser;
use cli::CliArgs;
use eframe::egui;
use fft::WindowFunction;
use palette::PaletteType;
use pipeline::{SpectrogramConfig, SpectrogramData};
use render::render_spectrogram_image;
use std::path::PathBuf;

fn parse_palette(s: &str) -> PaletteType {
    match s.to_lowercase().as_str() {
        "classic" => PaletteType::Classic,
        "magma" => PaletteType::Magma,
        "viridis" => PaletteType::Viridis,
        "inferno" => PaletteType::Inferno,
        "turbo" => PaletteType::Turbo,
        "gray" | "grey" | "grayscale" | "mono" => PaletteType::Grayscale,
        _ => PaletteType::Sox,
    }
}

fn parse_window(s: &str) -> WindowFunction {
    match s.to_lowercase().as_str() {
        "hamming" => WindowFunction::Hamming,
        "blackman" | "blackman-harris" => WindowFunction::BlackmanHarris,
        "flat" | "flattop" => WindowFunction::FlatTop,
        "rect" | "rectangular" => WindowFunction::Rectangular,
        _ => WindowFunction::Hann,
    }
}

fn parse_channel(s: &str) -> ChannelSelection {
    match s.to_lowercase().as_str() {
        "1" | "left" | "l" => ChannelSelection::Channel(0),
        "2" | "right" | "r" => ChannelSelection::Channel(1),
        _ => ChannelSelection::Mix,
    }
}

fn run_cli_headless(args: CliArgs, output_path: PathBuf) -> Result<()> {
    let input_path = args
        .input
        .ok_or_else(|| anyhow!("Please specify an input audio file or folder when using --output/-o"))?;

    let palette = parse_palette(&args.palette);

    // Batch folder mode
    if input_path.is_dir() {
        println!("📁 Scanning directory {:?} for audio files...", input_path);
        let files = scan_directory(&input_path);
        if files.is_empty() {
            return Err(anyhow!("No supported audio files found in {:?}", input_path));
        }

        println!("⚡ Batch processing {} files into {:?}...", files.len(), output_path);
        let res = batch_process_files(
            &files,
            &output_path,
            palette,
            args.width,
            args.height,
            |done, total| {
                print!("\r⏳ Processed {}/{} tracks...", done, total);
                use std::io::Write;
                let _ = std::io::stdout().flush();
            },
        );
        println!("\n✅ Batch complete: {} succeeded, {} failed.", res.succeeded, res.failed);
        return Ok(());
    }

    // Single file mode
    println!("🎵 Decoding audio from {:?}...", input_path);
    let channel_sel = parse_channel(&args.channel);
    let (info, samples) = decode_audio(&input_path, channel_sel, |p| {
        if (p * 100.0) as u32 % 25 == 0 {
            print!(".");
            use std::io::Write;
            let _ = std::io::stdout().flush();
        }
    })?;

    println!("\n📊 Audio Info:");
    println!("   Format:      {}", info.format_name);
    println!("   Codec:       {}", info.codec_name);
    println!("   Sample Rate: {} Hz", info.sample_rate);
    println!("   Channels:    {}", info.channels);
    println!("   Duration:    {:.2} s", info.duration_seconds);
    if let Some(br) = info.bitrate_kbps {
        println!("   Bitrate:     {} kbps", br);
    }

    let config = SpectrogramConfig {
        fft_size: args.fft,
        window_fn: parse_window(&args.window),
        num_columns: args.width.max(100) as usize,
        min_db: args.min_db,
        max_db: args.max_db,
    };

    println!("⚡ Computing FFT Spectrogram (FFT: {}, Rayon parallel)...", config.fft_size);
    let data = SpectrogramData::compute(info, &samples, config, |_| {});

    println!("🎨 Rendering image to {:?} ({}x{})...", output_path, args.width, args.height);
    let img = render_spectrogram_image(&data, palette, args.min_db, args.max_db, args.width, args.height);

    img.save(&output_path)
        .map_err(|e| anyhow!("Failed to save output image: {e}"))?;

    println!("✅ Spectrogram successfully saved to {:?}", output_path);
    Ok(())
}

fn main() -> Result<()> {
    env_logger::init();
    let args = CliArgs::parse();

    // Headless CLI mode
    if let Some(ref out_path) = args.output {
        let out = out_path.clone();
        return run_cli_headless(args, out);
    }

    // GUI mode
    let icon_data = if let Ok(image) = image::load_from_memory(include_bytes!("../assets/icon.png")) {
        let rgba = image.to_rgba8();
        let (width, height) = rgba.dimensions();
        Some(egui::IconData {
            rgba: rgba.into_raw(),
            width,
            height,
        })
    } else {
        None
    };

    let mut viewport = egui::ViewportBuilder::default()
        .with_title("Spekr - Acoustic Spectrum Analyser")
        .with_inner_size([1150.0, 720.0])
        .with_min_inner_size([680.0, 420.0])
        .with_drag_and_drop(true);

    if let Some(icon) = icon_data {
        viewport = viewport.with_icon(icon);
    }

    let native_options = eframe::NativeOptions {
        viewport,
        ..Default::default()
    };

    let initial_file = args.input;

    eframe::run_native(
        "Spekr",
        native_options,
        Box::new(|cc| {
            fonts::configure_egui_fonts(&cc.egui_ctx);
            Ok(Box::new(SpekrApp::new(initial_file)))
        }),
    )
    .map_err(|e| anyhow!("Failed to launch GUI: {e}"))
}
