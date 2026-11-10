use crate::audio::{decode_audio, AudioInfo, ChannelSelection};
use crate::batch::{batch_process_files, is_audio_file, scan_directory};
use crate::fft::WindowFunction;
use crate::palette::PaletteType;
use crate::playback::AudioPlayer;
use crate::pipeline::{SpectrogramConfig, SpectrogramData};
use crate::render::render_spectrogram_image;
use eframe::egui::{self, Color32, Pos2, Rect, Stroke, Vec2};
use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;

enum AppMessage {
    ProgressUpdate(usize, f32, String),
    TrackDecoded(usize, AudioInfo, Arc<Vec<f32>>), // 0: Track A, 1: Track B
    SpectrogramReady(usize, SpectrogramData),
    BatchProgress(usize, usize),
    BatchFinished(usize, usize),
    Error(String),
}

#[derive(Default)]
pub struct TrackView {
    pub file_path: Option<PathBuf>,
    pub audio_info: Option<AudioInfo>,
    pub cached_samples: Option<Arc<Vec<f32>>>,
    pub spectrogram_data: Option<SpectrogramData>,
    pub texture: Option<egui::TextureHandle>,
    pub cover_texture: Option<egui::TextureHandle>,

    // Zoom & Pan state (Normalized 0.0 to 1.0)
    pub time_min: f64,
    pub time_max: f64,
    pub freq_min: f32,
    pub freq_max: f32,

    pub is_processing: bool,
    pub progress: f32,
    pub status: String,
}

impl TrackView {
    pub fn reset_zoom(&mut self) {
        let dur = self.audio_info.as_ref().map(|i| i.duration_seconds).unwrap_or(1.0).max(0.1);
        let nyquist = self.spectrogram_data.as_ref().map(|d| d.nyquist_freq).unwrap_or(22050.0);
        self.time_min = 0.0;
        self.time_max = dur;
        self.freq_min = 0.0;
        self.freq_max = nyquist;
    }
}

pub struct SpekrApp {
    // Dual track slots for comparison (Track A = 0, Track B = 1)
    track_a: TrackView,
    track_b: TrackView,
    compare_mode: bool,
    active_slot: usize, // 0 or 1 for which slot opens next file
    playback_slot: usize, // 0 for Track A, 1 for Track B

    // Audio Playback Engine
    player: Arc<Mutex<AudioPlayer>>,
    volume: f32,

    // Playlist / Batch Folder
    playlist: Vec<PathBuf>,
    show_playlist: bool,
    is_batch_exporting: bool,
    batch_progress: (usize, usize),

    // Palette & Visual Settings
    palette: PaletteType,
    color_bar_texture: Option<egui::TextureHandle>,

    // Thread communication
    tx: Sender<AppMessage>,
    rx: Receiver<AppMessage>,
}

impl SpekrApp {
    pub fn new(initial_file: Option<PathBuf>) -> Self {
        let (tx, rx) = channel();
        let player = AudioPlayer::new();

        let mut app = Self {
            track_a: TrackView::default(),
            track_b: TrackView::default(),
            compare_mode: false,
            active_slot: 0,
            playback_slot: 0,
            player,
            volume: 1.0,
            playlist: Vec::new(),
            show_playlist: false,
            is_batch_exporting: false,
            batch_progress: (0, 0),
            palette: PaletteType::Sox,
            color_bar_texture: None,
            tx,
            rx,
        };

        app.track_a.reset_zoom();
        app.track_b.reset_zoom();

        if let Some(path) = initial_file {
            if path.is_dir() {
                app.load_folder(path);
            } else {
                app.load_file_into_slot(path, 0);
            }
        }

        app
    }

    pub fn load_file_into_slot(&mut self, path: PathBuf, slot: usize) {
        if slot == 0 {
            self.track_a.file_path = Some(path.clone());
            self.track_a.cover_texture = None;
            self.track_a.is_processing = true;
            self.track_a.progress = 0.0;
            self.track_a.status = format!("Decoding {}", path.file_name().and_then(|s| s.to_str()).map(crate::fonts::to_nfc).unwrap_or_default());
        } else {
            self.track_b.file_path = Some(path.clone());
            self.track_b.cover_texture = None;
            self.track_b.is_processing = true;
            self.track_b.progress = 0.0;
            self.track_b.status = format!("Decoding {}", path.file_name().and_then(|s| s.to_str()).map(crate::fonts::to_nfc).unwrap_or_default());
        }

        let tx = self.tx.clone();
        thread::spawn(move || {
            let tx_prog = tx.clone();
            let res = decode_audio(&path, ChannelSelection::Mix, move |p| {
                let _ = tx_prog.send(AppMessage::ProgressUpdate(slot, p * 0.45, "Decoding audio".into()));
            });
            match res {
                Ok((info, samples)) => {
                    let samples_arc = Arc::new(samples);
                    let _ = tx.send(AppMessage::TrackDecoded(slot, info, samples_arc));
                }
                Err(err) => {
                    let _ = tx.send(AppMessage::Error(format!("Error loading audio: {err}")));
                }
            }
        });
    }

    pub fn switch_playback_track(&mut self, target_slot: usize) {
        if self.playback_slot == target_slot {
            return;
        }
        self.playback_slot = target_slot;

        let (samples_opt, info_opt) = if target_slot == 0 {
            (&self.track_a.cached_samples, &self.track_a.audio_info)
        } else {
            (&self.track_b.cached_samples, &self.track_b.audio_info)
        };

        if let (Some(samples), Some(info)) = (samples_opt, info_opt) {
            if let Ok(mut player) = self.player.lock() {
                let cur_time = player.current_time();
                let was_playing = player.is_playing();
                player.load_track(Arc::clone(samples), 1, info.sample_rate, info.duration_seconds);
                if cur_time > 0.0 {
                    player.seek(cur_time);
                }
                if was_playing {
                    player.play();
                }
            }
        }
    }

    pub fn load_folder(&mut self, folder: PathBuf) {
        let files = scan_directory(&folder);
        if !files.is_empty() {
            let first = files[0].clone();
            self.playlist = files;
            self.show_playlist = true;
            self.load_file_into_slot(first, 0);
        }
    }

    pub fn play_prev_track(&mut self) {
        if self.playlist.is_empty() {
            return;
        }
        let current_path = self.track_a.file_path.as_ref();
        let current_idx = current_path
            .and_then(|cp| self.playlist.iter().position(|p| p == cp))
            .unwrap_or(0);
        let prev_idx = (current_idx + self.playlist.len() - 1) % self.playlist.len();
        let path = self.playlist[prev_idx].clone();
        self.load_file_into_slot(path, 0);
    }

    pub fn play_next_track(&mut self) {
        if self.playlist.is_empty() {
            return;
        }
        let current_path = self.track_a.file_path.as_ref();
        let current_idx = current_path
            .and_then(|cp| self.playlist.iter().position(|p| p == cp))
            .unwrap_or(0);
        let next_idx = (current_idx + 1) % self.playlist.len();
        let path = self.playlist[next_idx].clone();
        self.load_file_into_slot(path, 0);
    }

    fn compute_spectrogram_for_slot(&mut self, slot: usize) {
        let (info_opt, samples_opt) = if slot == 0 {
            (&self.track_a.audio_info, &self.track_a.cached_samples)
        } else {
            (&self.track_b.audio_info, &self.track_b.cached_samples)
        };

        if let (Some(info), Some(samples)) = (info_opt, samples_opt) {
            let tx = self.tx.clone();
            let info = info.clone();
            let samples = Arc::clone(samples);
            let config = SpectrogramConfig {
                fft_size: 2048,
                window_fn: WindowFunction::Hann,
                num_columns: 1200,
                min_db: -120.0,
                max_db: 0.0,
            };

            let tx_fft = tx.clone();
            thread::spawn(move || {
                let data = SpectrogramData::compute(info, &samples, config, move |p| {
                    let _ = tx_fft.send(AppMessage::ProgressUpdate(slot, 0.45 + p * 0.55, "Computing FFT".into()));
                });
                let _ = tx.send(AppMessage::SpectrogramReady(slot, data));
            });
        }
    }

    fn update_texture_for_slot(&mut self, slot: usize, ctx: &egui::Context) {
        let (data_opt, texture_slot) = if slot == 0 {
            (&self.track_a.spectrogram_data, &mut self.track_a.texture)
        } else {
            (&self.track_b.spectrogram_data, &mut self.track_b.texture)
        };

        if let Some(data) = data_opt {
            let width = data.matrix.len();
            let height = data.num_bins;
            if width == 0 || height == 0 {
                return;
            }

            let mut pixels = Vec::with_capacity(width * height * 4);
            let min_db = -120.0f32;
            let max_db = 0.0f32;
            let db_range = (max_db - min_db).max(1.0);

            for y in 0..height {
                let bin_idx = height - 1 - y;
                for x in 0..width {
                    let db = data.matrix[x][bin_idx];
                    let level = ((db - min_db) / db_range).clamp(0.0, 1.0);
                    let [r, g, b] = self.palette.sample(level);
                    pixels.push(r);
                    pixels.push(g);
                    pixels.push(b);
                    pixels.push(255);
                }
            }

            let color_image = egui::ColorImage::from_rgba_unmultiplied([width, height], &pixels);
            let tex_name = format!("spectrogram_slot_{}", slot);
            *texture_slot = Some(ctx.load_texture(tex_name, color_image, Default::default()));

            // Update color bar
            if self.color_bar_texture.is_none() {
                let bar_h = 256;
                let mut bar_pixels = Vec::with_capacity(bar_h * 4);
                for y in 0..bar_h {
                    let frac = (bar_h - 1 - y) as f32 / bar_h as f32;
                    let [r, g, b] = self.palette.sample(frac);
                    bar_pixels.push(r);
                    bar_pixels.push(g);
                    bar_pixels.push(b);
                    bar_pixels.push(255);
                }
                let bar_img = egui::ColorImage::from_rgba_unmultiplied([1, bar_h], &bar_pixels);
                self.color_bar_texture = Some(ctx.load_texture("colorbar", bar_img, Default::default()));
            }
        }
    }

    fn handle_messages(&mut self, ctx: &egui::Context) {
        while let Ok(msg) = self.rx.try_recv() {
            match msg {
                AppMessage::ProgressUpdate(slot, prog, status) => {
                    let track = if slot == 0 { &mut self.track_a } else { &mut self.track_b };
                    track.progress = prog;
                    track.status = status;
                }
                AppMessage::TrackDecoded(slot, info, samples) => {
                    let cover_tex = if let Some(bytes) = &info.cover_art {
                        if let Ok(dynamic_img) = image::load_from_memory(bytes) {
                            let rgba = dynamic_img.to_rgba8();
                            let (w, h) = (rgba.width() as usize, rgba.height() as usize);
                            let color_img = egui::ColorImage::from_rgba_unmultiplied([w, h], rgba.as_raw());
                            Some(ctx.load_texture(format!("cover_slot_{}", slot), color_img, Default::default()))
                        } else {
                            None
                        }
                    } else {
                        None
                    };

                    if slot == 0 {
                        self.track_a.cover_texture = cover_tex;
                        self.track_a.audio_info = Some(info.clone());
                        self.track_a.cached_samples = Some(samples.clone());
                        self.track_a.reset_zoom();
                        if self.playback_slot == 0 {
                            if let Ok(mut player) = self.player.lock() {
                                player.load_track(samples, 1, info.sample_rate, info.duration_seconds);
                            }
                        }
                    } else {
                        self.track_b.cover_texture = cover_tex;
                        self.track_b.audio_info = Some(info.clone());
                        self.track_b.cached_samples = Some(samples.clone());
                        self.track_b.reset_zoom();
                        if self.playback_slot == 1 {
                            if let Ok(mut player) = self.player.lock() {
                                player.load_track(samples, 1, info.sample_rate, info.duration_seconds);
                            }
                        }
                    }
                    self.compute_spectrogram_for_slot(slot);
                }
                AppMessage::SpectrogramReady(slot, data) => {
                    if slot == 0 {
                        self.track_a.spectrogram_data = Some(data);
                        self.track_a.is_processing = false;
                        self.track_a.progress = 1.0;
                        self.track_a.status = "Ready".into();
                        self.track_a.reset_zoom();
                    } else {
                        self.track_b.spectrogram_data = Some(data);
                        self.track_b.is_processing = false;
                        self.track_b.progress = 1.0;
                        self.track_b.status = "Ready".into();
                        self.track_b.reset_zoom();
                    }
                    self.update_texture_for_slot(slot, ctx);
                }
                AppMessage::BatchProgress(current, total) => {
                    self.batch_progress = (current, total);
                }
                AppMessage::BatchFinished(succeeded, total) => {
                    self.is_batch_exporting = false;
                    self.track_a.status = format!("Batch Export Finished! Saved {}/{} images.", succeeded, total);
                }
                AppMessage::Error(err) => {
                    self.track_a.is_processing = false;
                    self.track_b.is_processing = false;
                    self.track_a.status = err;
                }
            }
        }
    }

    fn render_spectrogram_slot(
        &mut self,
        ui: &mut egui::Ui,
        slot_idx: usize,
        canvas_rect: Rect,
        current_play_time: f64,
    ) {
        let is_active_playback = self.playback_slot == slot_idx;
        let mut seek_req: Option<f64> = None;

        {
            let (view, is_active) = if slot_idx == 0 {
                (&mut self.track_a, is_active_playback)
            } else {
                (&mut self.track_b, is_active_playback)
            };

        let painter = ui.painter();
        painter.rect_filled(canvas_rect, 0.0, Color32::from_rgb(12, 14, 18));

        let (total_duration, nyquist_freq) = match &view.spectrogram_data {
            Some(d) => (d.info.duration_seconds.max(0.1), d.nyquist_freq),
            None => {
                if view.is_processing {
                    let prog = view.progress.clamp(0.0, 1.0);
                    let pct = (prog * 100.0) as u32;

                    let center = canvas_rect.center();
                    let card_rect = Rect::from_center_size(center, Vec2::new(320.0, 68.0));
                    painter.rect_filled(card_rect, 6.0, Color32::from_rgb(22, 26, 34));
                    painter.rect_stroke(card_rect, 6.0, Stroke::new(1.0_f32, Color32::from_rgb(55, 65, 80)), egui::StrokeKind::Inside);

                    painter.text(
                        Pos2::new(center.x, center.y - 12.0),
                        egui::Align2::CENTER_CENTER,
                        format!("⚡ {} ({}%)...", view.status, pct),
                        egui::FontId::proportional(14.0),
                        Color32::from_rgb(0, 230, 255),
                    );

                    // Mini progress bar in canvas center
                    let bar_w = 260.0;
                    let bar_h = 6.0;
                    let bar_bg_rect = Rect::from_center_size(Pos2::new(center.x, center.y + 12.0), Vec2::new(bar_w, bar_h));
                    painter.rect_filled(bar_bg_rect, 3.0, Color32::from_rgb(40, 48, 62));
                    let fill_w = bar_w * prog;
                    if fill_w > 0.0 {
                        let bar_fill_rect = Rect::from_min_max(
                            bar_bg_rect.min,
                            Pos2::new(bar_bg_rect.min.x + fill_w, bar_bg_rect.max.y),
                        );
                        painter.rect_filled(bar_fill_rect, 3.0, Color32::from_rgb(0, 200, 255));
                    }
                } else {
                    let placeholder = if slot_idx == 0 {
                        "Drop an audio file/folder here or click 'Open Audio'"
                    } else {
                        "Click 'Load Track B' or drop file to compare"
                    };
                    painter.text(
                        canvas_rect.center(),
                        egui::Align2::CENTER_CENTER,
                        placeholder,
                        egui::FontId::proportional(16.0),
                        Color32::from_rgb(110, 120, 140),
                    );
                }
                return;
            }
        };

        // Calculate UV coordinates for zoom & pan
        let uv_x_min = (view.time_min / total_duration).clamp(0.0, 1.0) as f32;
        let uv_x_max = (view.time_max / total_duration).clamp(0.0, 1.0) as f32;
        let uv_y_min = 1.0 - (view.freq_max / nyquist_freq).clamp(0.0, 1.0);
        let uv_y_max = 1.0 - (view.freq_min / nyquist_freq).clamp(0.0, 1.0);

        if let Some(texture) = &view.texture {
            painter.image(
                texture.id(),
                canvas_rect,
                Rect::from_min_max(Pos2::new(uv_x_min, uv_y_min), Pos2::new(uv_x_max, uv_y_max)),
                Color32::WHITE,
            );
        }

        // Plot Border
        painter.rect_stroke(
            canvas_rect,
            0.0,
            Stroke::new(1.0_f32, Color32::from_rgb(70, 75, 85)),
            egui::StrokeKind::Inside,
        );

        // Dynamic Frequency Ruler (Y-axis on left)
        let visible_freq_min = view.freq_min / 1000.0;
        let visible_freq_max = view.freq_max / 1000.0;
        let freq_span = visible_freq_max - visible_freq_min;
        let freq_step = if freq_span <= 4.0 {
            0.5
        } else if freq_span <= 12.0 {
            1.0
        } else if freq_span <= 24.0 {
            2.0
        } else {
            5.0
        };

        let mut f = (visible_freq_min / freq_step).floor() * freq_step;
        while f <= visible_freq_max {
            if f >= visible_freq_min {
                let frac = (f - visible_freq_min) / freq_span;
                let y = canvas_rect.bottom() - frac * canvas_rect.height();

                painter.line_segment(
                    [Pos2::new(canvas_rect.left() - 4.0, y), Pos2::new(canvas_rect.left(), y)],
                    Stroke::new(1.0_f32, Color32::from_rgb(90, 95, 110)),
                );

                painter.text(
                    Pos2::new(canvas_rect.left() - 6.0, y),
                    egui::Align2::RIGHT_CENTER,
                    format!("{:.1}k", f),
                    egui::FontId::monospace(10.0),
                    Color32::from_rgb(175, 180, 195),
                );
            }
            f += freq_step;
        }

        // Dynamic Time Ruler (X-axis on bottom)
        let time_span = (view.time_max - view.time_min).max(0.1);
        let time_step = if time_span <= 10.0 {
            1.0
        } else if time_span <= 30.0 {
            5.0
        } else if time_span <= 120.0 {
            15.0
        } else {
            60.0
        };

        let mut t = (view.time_min / time_step).floor() * time_step;
        while t <= view.time_max {
            if t >= view.time_min {
                let frac = ((t - view.time_min) / time_span) as f32;
                let x = canvas_rect.left() + frac * canvas_rect.width();

                painter.line_segment(
                    [Pos2::new(x, canvas_rect.bottom()), Pos2::new(x, canvas_rect.bottom() + 4.0)],
                    Stroke::new(1.0_f32, Color32::from_rgb(90, 95, 110)),
                );

                let m = (t / 60.0) as u32;
                let s = (t % 60.0) as u32;
                painter.text(
                    Pos2::new(x, canvas_rect.bottom() + 6.0),
                    egui::Align2::CENTER_TOP,
                    format!("{:02}:{:02}", m, s),
                    egui::FontId::monospace(10.0),
                    Color32::from_rgb(175, 180, 195),
                );
            }
            t += time_step;
        }

        // Color Bar Legend (Right side)
        if let Some(bar_tex) = &self.color_bar_texture {
            let bar_rect = Rect::from_min_size(
                Pos2::new(canvas_rect.right() + 10.0, canvas_rect.top()),
                Vec2::new(12.0, canvas_rect.height()),
            );
            painter.image(
                bar_tex.id(),
                bar_rect,
                Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
            painter.rect_stroke(
                bar_rect,
                0.0,
                Stroke::new(1.0_f32, Color32::from_rgb(70, 75, 85)),
                egui::StrokeKind::Inside,
            );

            for &db_val in &[-120.0, -90.0, -60.0, -30.0, 0.0] {
                let frac = (db_val - -120.0) / 120.0;
                let y = canvas_rect.bottom() - frac * canvas_rect.height();
                painter.text(
                    Pos2::new(bar_rect.right() + 4.0, y),
                    egui::Align2::LEFT_CENTER,
                    format!("{:.0}", db_val),
                    egui::FontId::monospace(9.0),
                    Color32::from_rgb(160, 165, 180),
                );
            }
        }

        // Real-time Audio Playback Marker (Vertical Playhead Line)
        if (is_active || self.compare_mode) && current_play_time >= view.time_min && current_play_time <= view.time_max {
            let play_frac = ((current_play_time - view.time_min) / time_span) as f32;
            let play_x = canvas_rect.left() + play_frac * canvas_rect.width();

            // Glowing vertical playhead
            painter.line_segment(
                [Pos2::new(play_x, canvas_rect.top()), Pos2::new(play_x, canvas_rect.bottom())],
                Stroke::new(2.0_f32, if is_active { Color32::from_rgb(0, 240, 255) } else { Color32::from_rgb(80, 140, 160) }),
            );
        }

        // Mouse Hover Inspection & Zoom/Pan Interaction
        let response = ui.interact(canvas_rect, ui.id().with(slot_idx), egui::Sense::click_and_drag());

        let is_zoomed = (view.time_max - view.time_min) < total_duration - 0.05
            || view.freq_min > 50.0
            || view.freq_max < nyquist_freq - 50.0;

        if is_zoomed && response.hovered() {
            ui.ctx().set_cursor_icon(if response.dragged() {
                egui::CursorIcon::Grabbing
            } else {
                egui::CursorIcon::Grab
            });
        }

        // Drag to Pan (Left click or right click)
        if response.dragged() {
            let delta = response.drag_delta();
            if delta.length_sq() > 0.0 {
                let time_delta = -(delta.x as f64 / canvas_rect.width() as f64) * time_span;
                let freq_delta = (delta.y / canvas_rect.height()) * (view.freq_max - view.freq_min);

                let max_t_start = (total_duration - time_span).max(0.0);
                view.time_min = (view.time_min + time_delta).clamp(0.0, max_t_start);
                view.time_max = (view.time_min + time_span).min(total_duration);

                let freq_span_val = view.freq_max - view.freq_min;
                let max_f_start = (nyquist_freq - freq_span_val).max(0.0);
                view.freq_min = (view.freq_min + freq_delta).clamp(0.0, max_f_start);
                view.freq_max = (view.freq_min + freq_span_val).min(nyquist_freq);
            }
        }

        // Click to Seek (Only if user didn't drag)
        if response.clicked() && response.drag_delta().length() < 3.0 {
            if let Some(click_pos) = response.interact_pointer_pos() {
                let frac = (click_pos.x - canvas_rect.left()) / canvas_rect.width();
                let seek_time = view.time_min + frac as f64 * time_span;
                seek_req = Some(seek_time);
            }
        }

        // Double-click to Reset Zoom
        if response.double_clicked() {
            view.reset_zoom();
        }

        // Mouse Wheel Scroll: Zoom centered around cursor, or Shift+Scroll to Pan time
        if let Some(hover_pos) = ui.input(|i| i.pointer.hover_pos()) {
            if canvas_rect.contains(hover_pos) {
                let raw_scroll = ui.input(|i| i.raw_scroll_delta);
                let modifiers = ui.input(|i| i.modifiers);

                if modifiers.shift && raw_scroll.y.abs() > 0.1 {
                    // Shift + Scroll: Pan horizontally
                    let time_delta = -(raw_scroll.y as f64 / canvas_rect.width() as f64) * time_span * 0.5;
                    let max_t_start = (total_duration - time_span).max(0.0);
                    view.time_min = (view.time_min + time_delta).clamp(0.0, max_t_start);
                    view.time_max = (view.time_min + time_span).min(total_duration);
                } else if raw_scroll.y.abs() > 0.1 {
                    // Zoom centered around cursor
                    let zoom_factor = if raw_scroll.y > 0.0 { 0.85 } else { 1.15 };

                    // Zoom time
                    let mouse_t_frac = (hover_pos.x - canvas_rect.left()) / canvas_rect.width();
                    let mouse_time = view.time_min + mouse_t_frac as f64 * time_span;
                    let new_time_span = (time_span * zoom_factor).clamp(0.2, total_duration);
                    let max_t_start = (total_duration - new_time_span).max(0.0);
                    view.time_min = (mouse_time - mouse_t_frac as f64 * new_time_span).clamp(0.0, max_t_start);
                    view.time_max = (view.time_min + new_time_span).min(total_duration);

                    // Zoom frequency
                    let mouse_f_frac = (canvas_rect.bottom() - hover_pos.y) / canvas_rect.height();
                    let mouse_freq = view.freq_min + mouse_f_frac * (view.freq_max - view.freq_min);
                    let new_freq_span = ((view.freq_max - view.freq_min) * zoom_factor as f32).clamp(400.0, nyquist_freq);
                    let max_f_start = (nyquist_freq - new_freq_span).max(0.0);
                    view.freq_min = (mouse_freq - mouse_f_frac * new_freq_span).clamp(0.0, max_f_start);
                    view.freq_max = (view.freq_min + new_freq_span).min(nyquist_freq);
                }

                // Crosshair lines
                painter.line_segment(
                    [Pos2::new(canvas_rect.left(), hover_pos.y), Pos2::new(canvas_rect.right(), hover_pos.y)],
                    Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 70)),
                );
                painter.line_segment(
                    [Pos2::new(hover_pos.x, canvas_rect.top()), Pos2::new(hover_pos.x, canvas_rect.bottom())],
                    Stroke::new(1.0_f32, Color32::from_rgba_unmultiplied(255, 255, 255, 70)),
                );

                // HUD overlay
                let t_frac = (hover_pos.x - canvas_rect.left()) / canvas_rect.width();
                let hover_time = view.time_min + t_frac as f64 * time_span;

                let f_frac = (canvas_rect.bottom() - hover_pos.y) / canvas_rect.height();
                let hover_freq = view.freq_min + f_frac * (view.freq_max - view.freq_min);

                let db_val = view.spectrogram_data.as_ref().and_then(|d| d.get_value_at(hover_time, hover_freq)).unwrap_or(0.0);

                let tm = (hover_time / 60.0) as u32;
                let ts = (hover_time % 60.0) as u32;
                let tms = (hover_time.fract() * 100.0) as u32;

                let hud_text = format!(
                    "⏱ {:02}:{:02}.{:02}  |  🔊 {:.0} Hz ({:.2} kHz)  |  📊 {:.1} dB",
                    tm, ts, tms, hover_freq, hover_freq / 1000.0, db_val
                );

                let hud_pos = Pos2::new(
                    (hover_pos.x + 15.0).min(canvas_rect.right() - 260.0),
                    (hover_pos.y - 28.0).max(canvas_rect.top() + 6.0),
                );
                let hud_rect = Rect::from_min_size(hud_pos, Vec2::new(260.0, 22.0));
                painter.rect_filled(hud_rect, 4.0, Color32::from_rgba_unmultiplied(15, 18, 26, 230));
                painter.rect_stroke(
                    hud_rect,
                    4.0,
                    Stroke::new(1.0_f32, Color32::from_rgb(90, 100, 120)),
                    egui::StrokeKind::Inside,
                );
                painter.text(
                    hud_rect.center(),
                    egui::Align2::CENTER_CENTER,
                    hud_text,
                    egui::FontId::monospace(11.0),
                    Color32::from_rgb(255, 215, 80),
                );
            }
        }

        // Zoomed badge helper
        if is_zoomed {
            let badge_rect = Rect::from_min_size(
                Pos2::new(canvas_rect.left() + 8.0, canvas_rect.top() + 8.0),
                Vec2::new(310.0, 18.0),
            );
            painter.rect_filled(badge_rect, 3.0, Color32::from_rgba_unmultiplied(10, 14, 20, 210));
            painter.text(
                badge_rect.center(),
                egui::Align2::CENTER_CENTER,
                "🔍 Zoomed: Drag to Pan | Scroll to Zoom | Double-click Reset",
                egui::FontId::monospace(9.5),
                Color32::from_rgb(100, 210, 255),
            );
        }
        } // End of view borrow scope

        if let Some(seek_time) = seek_req {
            if self.playback_slot != slot_idx {
                self.switch_playback_track(slot_idx);
            }
            if let Ok(mut player) = self.player.lock() {
                player.seek(seek_time);
            }
        }
    }
}

impl eframe::App for SpekrApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_messages(ctx);

        // Handle Drag & Drop
        ctx.input(|i| {
            if !i.raw.dropped_files.is_empty() {
                if i.raw.dropped_files.len() == 1 {
                    if let Some(file) = i.raw.dropped_files.first() {
                        if let Some(path) = &file.path {
                            if path.is_dir() {
                                self.load_folder(path.clone());
                            } else if is_audio_file(path) {
                                if self.compare_mode && self.track_a.spectrogram_data.is_some() && self.active_slot == 1 {
                                    self.load_file_into_slot(path.clone(), 1);
                                } else {
                                    self.load_file_into_slot(path.clone(), 0);
                                }
                            }
                        }
                    }
                } else {
                    let mut audio_files = Vec::new();
                    for file in &i.raw.dropped_files {
                        if let Some(path) = &file.path {
                            if is_audio_file(path) {
                                audio_files.push(path.clone());
                            }
                        }
                    }
                    if !audio_files.is_empty() {
                        let first = audio_files[0].clone();
                        self.playlist = audio_files;
                        self.show_playlist = true;
                        self.load_file_into_slot(first, 0);
                    }
                }
            }
        });

        // Keyboard Shortcuts: Spacebar to toggle Play/Pause
        if ctx.input(|i| i.key_pressed(egui::Key::Space)) {
            if let Ok(mut player) = self.player.lock() {
                player.toggle_play_pause();
            }
        }

        // Current playback state
        let (is_playing, current_time) = if let Ok(player) = self.player.lock() {
            (player.is_playing(), player.current_time())
        } else {
            (false, 0.0)
        };
        let duration = if self.playback_slot == 0 {
            self.track_a.audio_info.as_ref().map(|i| i.duration_seconds).unwrap_or(0.0)
        } else {
            self.track_b.audio_info.as_ref().map(|i| i.duration_seconds).unwrap_or(0.0)
        };

        // Top Control Panel
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.heading("⚡ Spekr");
                ui.separator();

                if ui.button("📂 Open Audio").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("Audio Files", &["flac", "mp3", "wav", "m4a", "aac", "ogg", "alac", "aiff"])
                        .pick_file()
                    {
                        self.load_file_into_slot(path, 0);
                    }
                }

                if ui.button("📁 Open Folder").clicked() {
                    if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                        self.load_folder(folder);
                    }
                }

                let can_save = self.track_a.spectrogram_data.is_some();
                if ui.add_enabled(can_save, egui::Button::new("💾 Save PNG")).clicked() {
                    if let Some(data) = &self.track_a.spectrogram_data {
                        let default_name = format!("{}_spectrogram.png", data.info.file_name);
                        if let Some(path) = rfd::FileDialog::new()
                            .set_file_name(&default_name)
                            .add_filter("PNG Image", &["png"])
                            .save_file()
                        {
                            let img = render_spectrogram_image(data, self.palette, -120.0, 0.0, 1200, 600);
                            let _ = img.save(path);
                        }
                    }
                }

                ui.separator();

                // Feature 4: Compare Split-View Toggle
                let compare_label = if self.compare_mode { "🔀 Compare Mode (ON)" } else { "🔀 Compare Split-View" };
                if ui.selectable_label(self.compare_mode, compare_label).clicked() {
                    self.compare_mode = !self.compare_mode;
                    if self.compare_mode && self.track_b.spectrogram_data.is_none() {
                        self.active_slot = 1;
                    }
                }

                if self.compare_mode {
                    if ui.button("📂 Load Track B").clicked() {
                        if let Some(path) = rfd::FileDialog::new()
                            .add_filter("Audio Files", &["flac", "mp3", "wav", "m4a", "aac", "ogg", "alac", "aiff"])
                            .pick_file()
                        {
                            self.load_file_into_slot(path, 1);
                        }
                    }
                }

                ui.separator();

                // Feature 3: Reset Zoom
                if ui.button("🔍 Reset Zoom").clicked() {
                    self.track_a.reset_zoom();
                    self.track_b.reset_zoom();
                }

                // Playlist Sidebar Toggle
                if !self.playlist.is_empty() {
                    let btn_txt = if self.show_playlist { "📋 Hide Playlist" } else { "📋 Show Playlist" };
                    if ui.button(btn_txt).clicked() {
                        self.show_playlist = !self.show_playlist;
                    }
                }
            });
            ui.add_space(4.0);
        });

        // Bottom Foobar2000-Style Compact Player & Status Panel
        let has_audio = if self.playback_slot == 0 {
            self.track_a.cached_samples.is_some()
        } else {
            self.track_b.cached_samples.is_some()
        } && duration > 0.0;

        let cover_tex_id = if self.playback_slot == 0 {
            self.track_a.cover_texture.as_ref().map(|t| t.id())
        } else {
            self.track_b.cover_texture.as_ref().map(|t| t.id())
        };

        egui::TopBottomPanel::bottom("bottom_panel")
            .frame(
                egui::Frame::side_top_panel(ctx.style().as_ref())
                    .fill(Color32::from_rgb(236, 239, 244)) // Foobar2000 Classic Silver-Gray Light Panel
                    .stroke(Stroke::new(1.0_f32, Color32::from_rgb(195, 200, 210)))
                    .inner_margin(egui::Margin { left: 16, right: 16, top: 8, bottom: 8 })
            )
            .show(ctx, |ui| {
                *ui.visuals_mut() = egui::Visuals::light();
                // Row 1: Seek Progress Bar (Dài bằng chiều dài app, cách superView trái phải đúng 16px)
                let bar_width = ui.available_width();
                let bar_height = 6.0;
                let (bar_rect, bar_resp) = ui.allocate_exact_size(Vec2::new(bar_width, bar_height + 8.0), egui::Sense::click_and_drag());

                let track_rect = Rect::from_center_size(bar_rect.center(), Vec2::new(bar_rect.width(), bar_height));
                let painter = ui.painter();

                // 1. Rãnh ray trượt màu xám bạc rõ ràng (Foobar2000 style)
                painter.rect_filled(track_rect, 3.0, Color32::from_rgb(205, 212, 222));
                painter.rect_stroke(track_rect, 3.0, Stroke::new(1.0_f32, Color32::from_rgb(175, 184, 196)), egui::StrokeKind::Inside);

                let played_ratio = if duration > 0.0 {
                    (current_time / duration).clamp(0.0, 1.0) as f32
                } else {
                    0.0
                };

                let filled_w = track_rect.width() * played_ratio;
                if filled_w > 0.0 {
                    let fill_rect = Rect::from_min_max(
                        track_rect.min,
                        Pos2::new(track_rect.min.x + filled_w, track_rect.max.y),
                    );
                    let fill_color = if bar_resp.hovered() || bar_resp.dragged() {
                        Color32::from_rgb(0, 140, 240)
                    } else {
                        Color32::from_rgb(0, 115, 215)
                    };
                    painter.rect_filled(fill_rect, 3.0, fill_color);

                    // Scrubber thumb knob (Núm tròn trắng viền xanh nổi bật)
                    let thumb_radius = if bar_resp.hovered() || bar_resp.dragged() { 6.0 } else { 5.0 };
                    let thumb_center = Pos2::new(track_rect.min.x + filled_w, track_rect.center().y);
                    painter.circle_filled(thumb_center, thumb_radius + 1.0, Color32::from_rgb(0, 95, 185));
                    painter.circle_filled(thumb_center, thumb_radius, Color32::WHITE);
                }

                if has_audio && (bar_resp.clicked() || bar_resp.dragged()) {
                    if let Some(pos) = bar_resp.interact_pointer_pos() {
                        let frac = ((pos.x - track_rect.left()) / track_rect.width()).clamp(0.0, 1.0);
                        let seek_pos = frac as f64 * duration;
                        if let Ok(mut player) = self.player.lock() {
                            player.seek(seek_pos);
                        }
                    }
                }

                if bar_resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    if let Some(pos) = bar_resp.hover_pos() {
                        let hover_frac = ((pos.x - track_rect.left()) / track_rect.width()).clamp(0.0, 1.0);
                        let hover_sec = hover_frac as f64 * duration;
                        let hm = (hover_sec / 60.0) as u32;
                        let hs = (hover_sec % 60.0) as u32;
                        bar_resp.on_hover_text(format!("⏱ {:02}:{:02}", hm, hs));
                    }
                }

                ui.add_space(6.0);

                // Row 2: Controls + A/B Switcher + Digital LCD Box + Volume + Stream Metadata
                ui.horizontal(|ui| {
                    // 1. Album Cover Art Thumbnail (38x38)
                    if let Some(tex_id) = cover_tex_id {
                        ui.image((tex_id, Vec2::new(38.0, 38.0)));
                    } else {
                        let (rect, _) = ui.allocate_exact_size(Vec2::new(38.0, 38.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, 3.0, Color32::from_rgb(222, 226, 234));
                        ui.painter().rect_stroke(rect, 3.0, Stroke::new(1.0_f32, Color32::from_rgb(180, 188, 200)), egui::StrokeKind::Inside);
                        ui.painter().text(
                            rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "🎵",
                            egui::FontId::proportional(16.0),
                            Color32::from_rgb(100, 110, 125),
                        );
                    }

                    ui.add_space(2.0);

                    // 2. Track A / Track B Switcher (khi có 2 track hoặc ở Compare Mode)
                    if self.compare_mode || self.track_b.cached_samples.is_some() {
                        let a_selected = self.playback_slot == 0;
                        let a_btn = egui::Button::new("🅰 Track A")
                            .fill(if a_selected { Color32::from_rgb(0, 105, 210) } else { Color32::from_rgb(218, 224, 232) })
                            .min_size(Vec2::new(65.0, 24.0));
                        if ui.add(a_btn).on_hover_text("Nghe Track A").clicked() {
                            self.switch_playback_track(0);
                        }

                        let b_selected = self.playback_slot == 1;
                        let b_has_samples = self.track_b.cached_samples.is_some();
                        let b_btn = egui::Button::new("🅱 Track B")
                            .fill(if b_selected { Color32::from_rgb(215, 95, 0) } else { Color32::from_rgb(218, 224, 232) })
                            .min_size(Vec2::new(65.0, 24.0));
                        if ui.add_enabled(b_has_samples, b_btn).on_hover_text("Nghe Track B").clicked() {
                            self.switch_playback_track(1);
                        }
                        ui.separator();
                    }

                    // 3. Foobar2000 Transport Controls
                    let has_playlist = !self.playlist.is_empty();

                    if ui
                        .add_enabled(has_playlist, egui::Button::new("⏮").min_size(Vec2::new(26.0, 24.0)))
                        .on_hover_text("Previous track")
                        .clicked()
                    {
                        self.play_prev_track();
                    }

                    let play_icon = if is_playing { "⏸" } else { "▶" };
                    if ui
                        .add_enabled(has_audio, egui::Button::new(play_icon).min_size(Vec2::new(30.0, 24.0)))
                        .on_hover_text(if is_playing { "Pause (Space)" } else { "Play (Space)" })
                        .clicked()
                    {
                        if let Ok(mut player) = self.player.lock() {
                            player.toggle_play_pause();
                        }
                    }

                    if ui
                        .add_enabled(has_audio, egui::Button::new("⏹").min_size(Vec2::new(26.0, 24.0)))
                        .on_hover_text("Stop")
                        .clicked()
                    {
                        if let Ok(mut player) = self.player.lock() {
                            player.stop();
                        }
                    }

                    if ui
                        .add_enabled(has_playlist, egui::Button::new("⏭").min_size(Vec2::new(26.0, 24.0)))
                        .on_hover_text("Next track")
                        .clicked()
                    {
                        self.play_next_track();
                    }

                    ui.add_space(2.0);

                    // 4. Foobar2000 Digital LCD Screen with BLACK Text
                    let cur_m = (current_time / 60.0) as u32;
                    let cur_s = (current_time % 60.0) as u32;
                    let dur_m = (duration / 60.0) as u32;
                    let dur_s = (duration % 60.0) as u32;
                    let time_str = format!("{:02}:{:02} / {:02}:{:02}", cur_m, cur_s, dur_m, dur_s);

                    egui::Frame::canvas(ui.style())
                        .fill(Color32::from_rgb(215, 220, 225)) // Classic LCD grey-silver background
                        .stroke(Stroke::new(1.0_f32, Color32::from_rgb(135, 140, 150)))
                        .corner_radius(2.0)
                        .inner_margin(egui::Margin::symmetric(8, 3))
                        .show(ui, |ui| {
                            ui.monospace(
                                egui::RichText::new(time_str)
                                    .color(Color32::BLACK) // Yêu cầu: Thời gian chữ màu đen
                                    .strong()
                                    .size(12.5),
                            );
                        });

                    ui.add_space(2.0);

                    // 5. Volume Control (Dải 0% đến 100% max ở góc phải)
                    ui.label("🔈");
                    let vol_slider = egui::Slider::new(&mut self.volume, 0.0..=1.0).show_value(false);
                    if ui.add_sized([65.0, 16.0], vol_slider).changed() {
                        if let Ok(mut player) = self.player.lock() {
                            player.set_volume(self.volume);
                        }
                    }
                    ui.monospace(format!("{:>3.0}%", (self.volume * 100.0).round()));

                    ui.separator();

                    // 6. Audio Stream Technical Metadata
                    let active_info = if self.playback_slot == 0 {
                        self.track_a.audio_info.as_ref()
                    } else {
                        self.track_b.audio_info.as_ref()
                    };

                    let is_proc = self.track_a.is_processing || self.track_b.is_processing;
                    let (prog_val, prog_text) = if self.track_a.is_processing {
                        (self.track_a.progress, &self.track_a.status)
                    } else if self.track_b.is_processing {
                        (self.track_b.progress, &self.track_b.status)
                    } else {
                        (1.0, &self.track_a.status)
                    };

                    if is_proc {
                        let pct = (prog_val * 100.0).clamp(0.0, 100.0) as u32;
                        let bar_label = format!("{}: {}%", prog_text, pct);
                        ui.add_sized(
                            [210.0, 18.0],
                            egui::ProgressBar::new(prog_val.clamp(0.0, 1.0))
                                .text(egui::RichText::new(bar_label).size(11.0).color(Color32::from_rgb(20, 20, 20)))
                                .animate(true),
                        );
                    } else if let Some(info) = active_info {
                        let kbps_str = info
                            .bitrate_kbps
                            .map(|b| format!("{} kbps", b))
                            .unwrap_or_else(|| "VBR".into());
                        let bps_str = info
                            .bits_per_sample
                            .map(|b| format!("{}-bit | ", b))
                            .unwrap_or_default();
                        let ch_str = if info.channels == 1 {
                            "Mono"
                        } else if info.channels == 2 {
                            "Stereo"
                        } else {
                            "Multi-ch"
                        };

                        ui.label(
                            egui::RichText::new(format!(
                                "{} | {}{} Hz | {} | {}",
                                info.codec_name, bps_str, info.sample_rate, ch_str, kbps_str
                            ))
                            .color(Color32::from_rgb(45, 60, 80))
                            .monospace(),
                        );
                    }
                });
            });

        // Playlist Left Sidebar (Feature 5)
        if self.show_playlist && !self.playlist.is_empty() {
            egui::SidePanel::left("playlist_panel")
                .min_width(220.0)
                .max_width(320.0)
                .show(ctx, |ui| {
                    ui.heading(format!("📋 Album Playlist ({})", self.playlist.len()));
                    ui.separator();

                    // Batch Export All Button
                    if ui.add_enabled(!self.is_batch_exporting, egui::Button::new("⚡ Batch Export All to PNG")).clicked() {
                        if let Some(out_dir) = rfd::FileDialog::new().pick_folder() {
                            self.is_batch_exporting = true;
                            let files = self.playlist.clone();
                            let tx = self.tx.clone();
                            let palette = self.palette;

                            thread::spawn(move || {
                                let tx_prog = tx.clone();
                                let res = batch_process_files(&files, &out_dir, palette, 1200, 600, move |done, total| {
                                    let _ = tx_prog.send(AppMessage::BatchProgress(done, total));
                                });
                                let _ = tx.send(AppMessage::BatchFinished(res.succeeded, res.total));
                            });
                        }
                    }

                    if self.is_batch_exporting {
                        ui.add_space(4.0);
                        let prog = if self.batch_progress.1 > 0 {
                            self.batch_progress.0 as f32 / self.batch_progress.1 as f32
                        } else {
                            0.0
                        };
                        ui.add(egui::ProgressBar::new(prog).text(format!("Exporting {}/{}...", self.batch_progress.0, self.batch_progress.1)));
                    }

                    ui.separator();

                    // Scrollable File List
                    let mut selected_path: Option<PathBuf> = None;
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for path in &self.playlist {
                            let name = path
                                .file_name()
                                .and_then(|s| s.to_str())
                                .map(crate::fonts::to_nfc)
                                .unwrap_or_else(|| "track".to_string());
                            let is_current = self.track_a.file_path.as_ref() == Some(path);

                            if ui.selectable_label(is_current, &name).clicked() {
                                selected_path = Some(path.clone());
                            }
                        }
                    });

                    if let Some(path) = selected_path {
                        self.load_file_into_slot(path, self.active_slot);
                    }
                });
        }

        // Central Spectrogram Canvas
        egui::CentralPanel::default().show(ctx, |ui| {
            let available = ui.available_rect_before_wrap();
            let left_ruler_w = 55.0;
            let bottom_ruler_h = 24.0;
            let right_legend_w = 60.0;

            if !self.compare_mode {
                // Single View Mode
                let canvas_rect = Rect::from_min_max(
                    Pos2::new(available.left() + left_ruler_w, available.top() + 6.0),
                    Pos2::new(available.right() - right_legend_w, available.bottom() - bottom_ruler_h),
                );
                self.render_spectrogram_slot(ui, 0, canvas_rect, current_time);
            } else {
                // Split-View Compare Mode (Top: Track A, Bottom: Track B)
                let half_h = (available.height() - 20.0) / 2.0;

                // Track A (Top)
                ui.painter().text(
                    Pos2::new(available.left() + left_ruler_w, available.top()),
                    egui::Align2::LEFT_TOP,
                    format!("🅰 Track A: {}", self.track_a.audio_info.as_ref().map(|i| i.file_name.as_str()).unwrap_or("No File")),
                    egui::FontId::monospace(12.0),
                    Color32::from_rgb(100, 200, 255),
                );

                let canvas_rect_a = Rect::from_min_max(
                    Pos2::new(available.left() + left_ruler_w, available.top() + 18.0),
                    Pos2::new(available.right() - right_legend_w, available.top() + 18.0 + half_h - bottom_ruler_h),
                );
                self.render_spectrogram_slot(ui, 0, canvas_rect_a, current_time);

                // Track B (Bottom)
                let bottom_start_y = available.top() + 24.0 + half_h;
                ui.painter().text(
                    Pos2::new(available.left() + left_ruler_w, bottom_start_y - 14.0),
                    egui::Align2::LEFT_TOP,
                    format!("🅱 Track B: {}", self.track_b.audio_info.as_ref().map(|i| i.file_name.as_str()).unwrap_or("No File")),
                    egui::FontId::monospace(12.0),
                    Color32::from_rgb(255, 180, 100),
                );

                let canvas_rect_b = Rect::from_min_max(
                    Pos2::new(available.left() + left_ruler_w, bottom_start_y),
                    Pos2::new(available.right() - right_legend_w, available.bottom() - bottom_ruler_h),
                );
                self.render_spectrogram_slot(ui, 1, canvas_rect_b, current_time);
            }
        });

        // Request repaint while playing or processing (capped at ~60fps to prevent high CPU usage)
        if is_playing || self.track_a.is_processing || self.track_b.is_processing || self.is_batch_exporting {
            ctx.request_repaint_after(std::time::Duration::from_millis(16));
        }
    }
}
