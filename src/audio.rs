use std::fs::File;
use std::path::{Path, PathBuf};
use anyhow::{anyhow, Context, Result};
use symphonia::core::audio::SampleBuffer;
use symphonia::core::codecs::{
    CODEC_TYPE_AAC, CODEC_TYPE_ALAC, CODEC_TYPE_FLAC, CODEC_TYPE_MP3, CODEC_TYPE_NULL,
    CODEC_TYPE_OPUS, CODEC_TYPE_PCM_S16LE, CODEC_TYPE_VORBIS, DecoderOptions,
};
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct AudioInfo {
    pub path: PathBuf,
    pub file_name: String,
    pub format_name: String,
    pub codec_name: String,
    pub sample_rate: u32,
    pub channels: usize,
    pub bits_per_sample: Option<u32>,
    pub bitrate_kbps: Option<u32>,
    pub duration_seconds: f64,
    pub total_samples: u64,
    pub cover_art: Option<Vec<u8>>,
}

fn get_human_codec_name(codec: symphonia::core::codecs::CodecType) -> String {
    if codec == CODEC_TYPE_MP3 {
        "MP3".to_string()
    } else if codec == CODEC_TYPE_FLAC {
        "FLAC".to_string()
    } else if codec == CODEC_TYPE_AAC {
        "AAC".to_string()
    } else if codec == CODEC_TYPE_VORBIS {
        "Vorbis (OGG)".to_string()
    } else if codec == CODEC_TYPE_ALAC {
        "ALAC (Apple Lossless)".to_string()
    } else if codec == CODEC_TYPE_OPUS {
        "Opus".to_string()
    } else if codec == CODEC_TYPE_PCM_S16LE || format!("{:?}", codec).contains("PCM") {
        "PCM (Uncompressed)".to_string()
    } else {
        format!("{:?}", codec)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChannelSelection {
    #[default]
    Mix,
    Channel(usize),
}

pub fn decode_audio<F>(
    path: &Path,
    channel_sel: ChannelSelection,
    mut progress_cb: F,
) -> Result<(AudioInfo, Vec<f32>)>
where
    F: FnMut(f32),
{
    let file = File::open(path).with_context(|| format!("Failed to open file {:?}", path))?;
    let file_len = file.metadata().map(|m| m.len()).unwrap_or(0);
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|s| s.to_str()) {
        hint.with_extension(ext);
    }

    let format_opts = FormatOptions {
        enable_gapless: true,
        ..Default::default()
    };
    let metadata_opts: MetadataOptions = Default::default();

    let probed = symphonia::default::get_probe()
        .format(&hint, mss, &format_opts, &metadata_opts)
        .map_err(|e| anyhow!("Failed to probe audio format: {e}"))?;

    let mut format = probed.format;

    // Find the first audio track
    let track = format
        .tracks()
        .iter()
        .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
        .ok_or_else(|| anyhow!("No supported audio track found in file"))?;

    let track_id = track.id;
    let codec_params = track.codec_params.clone();
    let sample_rate = codec_params.sample_rate.unwrap_or(44100);
    let channels = codec_params.channels.map(|c| c.count()).unwrap_or(2);
    let bits_per_sample = codec_params.bits_per_sample;

    let codec_name = get_human_codec_name(codec_params.codec);
    let format_name = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("Audio")
        .to_uppercase();

    // Estimate duration
    let n_frames = codec_params.n_frames;
    let duration_seconds = if let Some(frames) = n_frames {
        frames as f64 / sample_rate as f64
    } else {
        0.0
    };

    // Extract embedded Album Art or folder cover image
    let mut cover_art: Option<Vec<u8>> = None;
    if let Some(rev) = format.metadata().current() {
        if let Some(visual) = rev.visuals().first() {
            cover_art = Some(visual.data.to_vec());
        }
    }
    if cover_art.is_none() {
        if let Some(parent) = path.parent() {
            for name in &["cover.jpg", "folder.jpg", "front.jpg", "cover.png", "folder.png", "front.png"] {
                let candidate = parent.join(name);
                if candidate.exists() {
                    if let Ok(bytes) = std::fs::read(&candidate) {
                        cover_art = Some(bytes);
                        break;
                    }
                }
            }
        }
    }

    let decoder_opts = DecoderOptions {
        verify: false,
        ..Default::default()
    };
    let mut decoder = symphonia::default::get_codecs()
        .make(&codec_params, &decoder_opts)
        .map_err(|e| anyhow!("Unsupported audio codec: {e}"))?;

    let mut samples_out: Vec<f32> = if let Some(frames) = n_frames {
        Vec::with_capacity((frames as usize).min(50_000_000))
    } else {
        Vec::with_capacity(sample_rate as usize * 180)
    };
    let mut sample_buf: Option<SampleBuffer<f32>> = None;
    let mut total_decoded_frames: u64 = 0;

    let total_expected_frames = n_frames.unwrap_or(0);

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            Err(SymphoniaError::IoError(err))
                if err.kind() == std::io::ErrorKind::UnexpectedEof =>
            {
                break;
            }
            Err(SymphoniaError::ResetRequired) => {
                decoder.reset();
                continue;
            }
            Err(err) => {
                log::warn!("Audio packet read finished or error: {err}");
                break;
            }
        };

        if packet.track_id() != track_id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(decoded) => {
                let spec = *decoded.spec();
                let capacity = decoded.capacity();

                if sample_buf.is_none() {
                    sample_buf = Some(SampleBuffer::new(capacity as u64, spec));
                }

                if let Some(buf) = sample_buf.as_mut() {
                    buf.copy_interleaved_ref(decoded);
                    let samples = buf.samples();
                    let ch_count = spec.channels.count();
                    let frames = samples.len() / ch_count;
                    total_decoded_frames += frames as u64;

                    match channel_sel {
                        ChannelSelection::Mix => {
                            if ch_count == 1 {
                                samples_out.extend_from_slice(samples);
                            } else {
                                samples_out.reserve(frames);
                                let inv_ch = 1.0f32 / ch_count as f32;
                                for chunk in samples.chunks_exact(ch_count) {
                                    let sum: f32 = chunk.iter().sum();
                                    samples_out.push(sum * inv_ch);
                                }
                            }
                        }
                        ChannelSelection::Channel(target_ch) => {
                            let ch = target_ch.min(ch_count.saturating_sub(1));
                            samples_out.reserve(frames);
                            for chunk in samples.chunks_exact(ch_count) {
                                samples_out.push(chunk[ch]);
                            }
                        }
                    }
                }
            }
            Err(SymphoniaError::DecodeError(err)) => {
                log::warn!("Decode warning: {err}");
            }
            Err(err) => {
                log::warn!("Decoder error: {err}");
                break;
            }
        }

        if total_expected_frames > 0 {
            let prog = (total_decoded_frames as f32 / total_expected_frames as f32).clamp(0.0, 1.0);
            progress_cb(prog);
        }
    }

    let actual_duration = if duration_seconds > 0.0 {
        duration_seconds
    } else {
        total_decoded_frames as f64 / sample_rate as f64
    };

    let bitrate_kbps = if actual_duration > 0.0 && file_len > 0 {
        Some(((file_len as f64 * 8.0) / (actual_duration * 1000.0)).round() as u32)
    } else {
        None
    };

    let info = AudioInfo {
        path: path.to_path_buf(),
        file_name: path
            .file_name()
            .and_then(|s| s.to_str())
            .map(crate::fonts::to_nfc)
            .unwrap_or_else(|| "unknown".to_string()),
        format_name,
        codec_name,
        sample_rate,
        channels,
        bits_per_sample,
        bitrate_kbps,
        duration_seconds: actual_duration,
        total_samples: samples_out.len() as u64,
        cover_art,
    };

    Ok((info, samples_out))
}
