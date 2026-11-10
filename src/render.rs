use crate::fonts::{get_fallback_font_bytes, BE_VIETNAM_PRO_REGULAR};
use crate::palette::PaletteType;
use crate::pipeline::SpectrogramData;
use ab_glyph::{point, Font, FontRef, PxScale, ScaleFont};
use image::{Rgb, RgbImage};

/// Renders a complete Spek-style spectrogram image with rulers, legends, and metadata
pub fn render_spectrogram_image(
    data: &SpectrogramData,
    palette: PaletteType,
    min_db: f32,
    max_db: f32,
    width: u32,
    height: u32,
) -> RgbImage {
    let width = width.max(500);
    let height = height.max(300);

    let mut img = RgbImage::from_pixel(width, height, Rgb([18, 20, 24])); // Dark slate background

    // Margins (like Spek LPAD, TPAD, RPAD, BPAD)
    let left_pad = 70u32;
    let right_pad = 80u32;
    let top_pad = 58u32;
    let bottom_pad = 40u32;

    if width <= left_pad + right_pad || height <= top_pad + bottom_pad {
        return img;
    }

    let plot_x = left_pad;
    let plot_y = top_pad;
    let plot_w = width - left_pad - right_pad;
    let plot_h = height - top_pad - bottom_pad;

    let num_cols = data.matrix.len();
    let num_bins = data.num_bins;
    let db_range = (max_db - min_db).max(1.0);

    // 1. Render Spectrogram Canvas
    if num_cols > 0 && num_bins > 0 {
        for px in 0..plot_w {
            let col_idx = ((px as f64 / plot_w as f64) * num_cols as f64) as usize;
            let col_idx = col_idx.min(num_cols - 1);
            let col_data = &data.matrix[col_idx];

            for py in 0..plot_h {
                // py = 0 is top (high frequency), py = plot_h - 1 is bottom (0 Hz)
                let bin_idx = (((plot_h - 1 - py) as f64 / plot_h as f64) * num_bins as f64) as usize;
                let bin_idx = bin_idx.min(num_bins - 1);

                let db = col_data[bin_idx];
                let level = ((db - min_db) / db_range).clamp(0.0, 1.0);
                let rgb = palette.sample(level);

                img.put_pixel(plot_x + px, plot_y + py, Rgb(rgb));
            }
        }
    }

    // 2. Draw border around plot
    let border_color = Rgb([70, 75, 85]);
    for x in plot_x..plot_x + plot_w {
        img.put_pixel(x, plot_y - 1, border_color);
        img.put_pixel(x, plot_y + plot_h, border_color);
    }
    for y in plot_y..plot_y + plot_h {
        img.put_pixel(plot_x - 1, y, border_color);
        img.put_pixel(plot_x + plot_w, y, border_color);
    }

    // 3. Render Frequency Ruler (Y-axis, on left)
    let nyquist_khz = data.nyquist_freq / 1000.0;
    let freq_step_khz = if nyquist_khz <= 24.0 {
        2.0
    } else if nyquist_khz <= 48.0 {
        4.0
    } else {
        10.0
    };

    let mut current_f = 0.0f32;
    while current_f <= nyquist_khz {
        let frac = current_f / nyquist_khz;
        let y = (plot_y + plot_h).saturating_sub((frac * plot_h as f32) as u32);
        if y >= plot_y && y <= plot_y + plot_h {
            // Tick mark
            for tx in (plot_x.saturating_sub(5))..plot_x {
                img.put_pixel(tx, y, border_color);
            }
            // Label
            let label = format!("{:>2.0} kHz", current_f);
            draw_text(&mut img, plot_x.saturating_sub(62), y.saturating_sub(6), &label, Rgb([180, 185, 195]), 11.0);
        }
        current_f += freq_step_khz;
    }

    // 4. Render Time Ruler (X-axis, on bottom)
    let duration = data.info.duration_seconds;
    if duration > 0.0 {
        let time_step = if duration <= 60.0 {
            10.0
        } else if duration <= 300.0 {
            30.0
        } else if duration <= 600.0 {
            60.0
        } else if duration <= 1800.0 {
            120.0
        } else {
            300.0
        };

        let mut current_t = 0.0f64;
        while current_t <= duration {
            let frac = current_t / duration;
            let x = plot_x + (frac * plot_w as f64) as u32;
            if x <= plot_x + plot_w {
                // Tick mark
                for ty in (plot_y + plot_h)..(plot_y + plot_h + 5) {
                    img.put_pixel(x, ty, border_color);
                }
                // Label mm:ss
                let mins = (current_t / 60.0) as u32;
                let secs = (current_t % 60.0) as u32;
                let label = format!("{:02}:{:02}", mins, secs);
                let label_x = x.saturating_sub(18).max(plot_x);
                draw_text(&mut img, label_x, plot_y + plot_h + 8, &label, Rgb([180, 185, 195]), 11.0);
            }
            current_t += time_step;
        }
    }

    // 5. Render Color Scale / dB Bar (on right)
    let bar_x = plot_x + plot_w + 14;
    let bar_w = 12u32;
    for py in 0..plot_h {
        let frac = (plot_h - 1 - py) as f32 / plot_h as f32;
        let rgb = palette.sample(frac);
        for bx in 0..bar_w {
            img.put_pixel(bar_x + bx, plot_y + py, Rgb(rgb));
        }
    }
    // Bar border
    for bx in bar_x..bar_x + bar_w {
        img.put_pixel(bx, plot_y - 1, border_color);
        img.put_pixel(bx, plot_y + plot_h, border_color);
    }
    for by in plot_y..plot_y + plot_h {
        img.put_pixel(bar_x - 1, by, border_color);
        img.put_pixel(bar_x + bar_w, by, border_color);
    }

    // dB labels
    let db_ticks = [-120.0, -100.0, -80.0, -60.0, -40.0, -20.0, 0.0];
    for &val in &db_ticks {
        if val >= min_db && val <= max_db {
            let frac = (val - min_db) / db_range;
            let y = (plot_y + plot_h).saturating_sub((frac * plot_h as f32) as u32);
            if y >= plot_y && y <= plot_y + plot_h {
                for tx in (bar_x + bar_w)..(bar_x + bar_w + 4) {
                    img.put_pixel(tx, y, border_color);
                }
                let label = format!("{:>4.0} dB", val);
                draw_text(&mut img, bar_x + bar_w + 7, y.saturating_sub(6), &label, Rgb([180, 185, 195]), 11.0);
            }
        }
    }

    // 6. Header Metadata (Top)
    let file_title = format!("Spekr  -  {}", data.info.file_name);
    draw_text(&mut img, plot_x, 10, &file_title, Rgb([240, 240, 245]), 15.0);

    let dur_mins = (data.info.duration_seconds / 60.0) as u32;
    let dur_secs = (data.info.duration_seconds % 60.0) as u32;
    let dur_ms = ((data.info.duration_seconds.fract()) * 100.0) as u32;

    let sub_info = format!(
        "{} | {} ch | {} Hz | {} | {:02}:{:02}.{:02}",
        data.info.codec_name,
        data.info.channels,
        data.info.sample_rate,
        data.info.bitrate_kbps.map(|b| format!("{} kbps", b)).unwrap_or_else(|| "VBR".into()),
        dur_mins,
        dur_secs,
        dur_ms
    );
    draw_text(&mut img, plot_x, 32, &sub_info, Rgb([140, 150, 170]), 12.0);

    img
}

/// Crisp, anti-aliased text rendering with Google Font Be Vietnam Pro + CJK / Unicode fallback
fn draw_text(img: &mut RgbImage, x: u32, y: u32, text: &str, color: Rgb<u8>, font_size: f32) {
    let nfc_text = crate::fonts::to_nfc(text);
    let primary_font = FontRef::try_from_slice(BE_VIETNAM_PRO_REGULAR).ok();
    let fallback_font = get_fallback_font_bytes().and_then(|b| FontRef::try_from_slice(b).ok());

    let scale = PxScale::from(font_size);
    let mut cursor_x = x as f32;

    for ch in nfc_text.chars() {
        if ch == '\n' {
            continue;
        }

        // Determine which font has this glyph
        let mut chosen_font: Option<&FontRef> = None;
        let mut chosen_gid = None;

        if let Some(font) = &primary_font {
            let gid = font.glyph_id(ch);
            if gid.0 != 0 {
                chosen_font = Some(font);
                chosen_gid = Some(gid);
            }
        }

        if chosen_font.is_none() {
            if let Some(font) = &fallback_font {
                let gid = font.glyph_id(ch);
                if gid.0 != 0 {
                    chosen_font = Some(font);
                    chosen_gid = Some(gid);
                }
            }
        }

        let font = chosen_font.or(primary_font.as_ref());
        if let Some(font) = font {
            let gid = chosen_gid.unwrap_or_else(|| font.glyph_id(ch));
            let scaled_font = font.as_scaled(scale);
            let baseline_y = y as f32 + scaled_font.ascent();
            let glyph = gid.with_scale_and_position(scale, point(cursor_x, baseline_y));

            if let Some(outlined) = font.outline_glyph(glyph) {
                let bounds = outlined.px_bounds();
                outlined.draw(|gx, gy, coverage| {
                    if coverage <= 0.01 {
                        return;
                    }
                    let px = bounds.min.x as i32 + gx as i32;
                    let py = bounds.min.y as i32 + gy as i32;

                    if px >= 0 && px < img.width() as i32 && py >= 0 && py < img.height() as i32 {
                        let px = px as u32;
                        let py = py as u32;
                        let orig = img.get_pixel(px, py);
                        let c = coverage.clamp(0.0, 1.0);
                        let r = (orig.0[0] as f32 * (1.0 - c) + color.0[0] as f32 * c).round() as u8;
                        let g = (orig.0[1] as f32 * (1.0 - c) + color.0[1] as f32 * c).round() as u8;
                        let b = (orig.0[2] as f32 * (1.0 - c) + color.0[2] as f32 * c).round() as u8;
                        img.put_pixel(px, py, Rgb([r, g, b]));
                    }
                });
            }

            cursor_x += scaled_font.h_advance(gid);
        } else {
            cursor_x += font_size * 0.6;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_draw_text_vietnamese_and_chinese() {
        let mut img = RgbImage::new(500, 100);
        // Test with raw NFD (decomposed) string simulating macOS filesystem input
        let nfd_sample = "Ba\u{0300}i ha\u{0301}t tie\u{0302}\u{0301}ng Vie\u{0302}\u{0323}t - 晴天 (Jay Chou)";
        draw_text(&mut img, 10, 10, nfd_sample, Rgb([255, 255, 255]), 18.0);
        let painted_pixels = img.pixels().filter(|p| p.0 != [0, 0, 0]).count();
        println!("Painted pixels count (from NFD input): {}", painted_pixels);
        assert!(painted_pixels > 100, "Text was not painted!");
    }
}
