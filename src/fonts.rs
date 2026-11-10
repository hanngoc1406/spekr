use std::path::Path;
use std::sync::OnceLock;
use unicode_normalization::UnicodeNormalization;

pub const BE_VIETNAM_PRO_REGULAR: &[u8] = include_bytes!("../assets/fonts/BeVietnamPro-Regular.ttf");
#[allow(dead_code)]
pub const BE_VIETNAM_PRO_MEDIUM: &[u8] = include_bytes!("../assets/fonts/BeVietnamPro-Medium.ttf");

/// Normalizes any string to Unicode NFC (Normalization Form C)
/// This is critical on macOS where filenames and inputs are decomposed into NFD
/// (e.g. "a" + combining grave accent), which breaks text layout in non-HarfBuzz renderers.
pub fn to_nfc(s: &str) -> String {
    s.nfc().collect()
}

static FALLBACK_FONT_DATA: OnceLock<Option<Vec<u8>>> = OnceLock::new();

pub fn get_fallback_font_bytes() -> Option<&'static [u8]> {
    FALLBACK_FONT_DATA
        .get_or_init(|| {
            let candidates = [
                // macOS
                "/System/Library/Fonts/Supplemental/Arial Unicode.ttf",
                "/Library/Fonts/Arial Unicode.ttf",
                "/System/Library/Fonts/STHeiti Light.ttc",
                "/System/Library/Fonts/Hiragino Sans GB.ttc",
                // Windows
                "C:\\Windows\\Fonts\\msyh.ttc",
                "C:\\Windows\\Fonts\\msyhl.ttc",
                "C:\\Windows\\Fonts\\simsun.ttc",
                "C:\\Windows\\Fonts\\arialuni.ttf",
                "C:\\Windows\\Fonts\\arial.ttf",
                // Linux
                "/usr/share/fonts/opentype/noto/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc",
                "/usr/share/fonts/truetype/wqy/wqy-microhei.ttc",
                "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf",
            ];

            for path_str in &candidates {
                let path = Path::new(path_str);
                if path.exists() {
                    if let Ok(data) = std::fs::read(path) {
                        return Some(data);
                    }
                }
            }
            None
        })
        .as_deref()
}

/// Setup Google Font Be Vietnam Pro + CJK Fallback for egui GUI
pub fn configure_egui_fonts(ctx: &eframe::egui::Context) {
    let mut fonts = eframe::egui::FontDefinitions::default();

    // 1. Google Font Be Vietnam Pro as Primary
    fonts.font_data.insert(
        "be_vietnam_pro".to_owned(),
        std::sync::Arc::new(eframe::egui::FontData::from_static(BE_VIETNAM_PRO_REGULAR)),
    );

    // 2. CJK / Unicode fallback
    if let Some(font_bytes) = get_fallback_font_bytes() {
        fonts.font_data.insert(
            "cjk_fallback".to_owned(),
            std::sync::Arc::new(eframe::egui::FontData::from_owned(font_bytes.to_vec())),
        );
    }

    let prop = fonts.families.entry(eframe::egui::FontFamily::Proportional).or_default();
    prop.insert(0, "be_vietnam_pro".to_owned());
    if fonts.font_data.contains_key("cjk_fallback") {
        prop.insert(1, "cjk_fallback".to_owned());
    }

    let mono = fonts.families.entry(eframe::egui::FontFamily::Monospace).or_default();
    mono.insert(0, "be_vietnam_pro".to_owned());
    if fonts.font_data.contains_key("cjk_fallback") {
        mono.insert(1, "cjk_fallback".to_owned());
    }

    ctx.set_fonts(fonts);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fonts_load() {
        let fallback = get_fallback_font_bytes();
        println!("Fallback exists: {}", fallback.is_some());
        if let Some(bytes) = fallback {
            let font = ab_glyph::FontRef::try_from_slice(bytes);
            assert!(font.is_ok(), "Fallback font must be valid TTF/TTC");
            if let Ok(f) = font {
                use ab_glyph::Font;
                // Fallback font must be a valid font containing standard glyphs
                assert_ne!(f.glyph_id('A').0, 0);
            }
        }

        let ctx = eframe::egui::Context::default();
        configure_egui_fonts(&ctx);
        let _ = ctx.run(Default::default(), |ctx| {
            // Test that NFD decomposed text normalized to NFC renders proper precomposed glyphs
            let nfd_input = "Ba\u{0300}i ha\u{0301}t tie\u{0302}\u{0301}ng Vie\u{0302}\u{0323}t";
            let nfc_text = to_nfc(nfd_input);
            ctx.fonts(|f| {
                let galley = f.layout_no_wrap(nfc_text, eframe::egui::FontId::proportional(14.0), eframe::egui::Color32::WHITE);
                assert_eq!(galley.rows.len(), 1);
                // Verify 'à', 'á', 'ế', 'ệ' are laid out as unified glyphs
                let chars: Vec<char> = galley.rows[0].glyphs.iter().map(|g| g.chr).collect();
                assert!(chars.contains(&'à'));
                assert!(chars.contains(&'á'));
                assert!(chars.contains(&'ế'));
                assert!(chars.contains(&'ệ'));
            });
        });
    }
}
