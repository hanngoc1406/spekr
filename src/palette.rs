use std::f32::consts::PI;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[allow(dead_code)]
pub enum PaletteType {
    Classic,
    #[default]
    Sox,
    Magma,
    Viridis,
    Inferno,
    Turbo,
    Grayscale,
}

#[allow(dead_code)]
impl PaletteType {
    pub const ALL: &'static [PaletteType] = &[
        PaletteType::Classic,
        PaletteType::Sox,
        PaletteType::Magma,
        PaletteType::Viridis,
        PaletteType::Inferno,
        PaletteType::Turbo,
        PaletteType::Grayscale,
    ];

    pub fn name(&self) -> &'static str {
        match self {
            PaletteType::Classic => "Spek Classic",
            PaletteType::Sox => "SoX",
            PaletteType::Magma => "Magma",
            PaletteType::Viridis => "Viridis",
            PaletteType::Inferno => "Inferno",
            PaletteType::Turbo => "Turbo",
            PaletteType::Grayscale => "Grayscale",
        }
    }

    /// Map a normalized level [0.0, 1.0] to RGB [u8; 3]
    pub fn sample(&self, level: f32) -> [u8; 3] {
        let level = level.clamp(0.0, 1.0);
        match self {
            PaletteType::Classic => palette_classic(level),
            PaletteType::Sox => palette_sox(level),
            PaletteType::Magma => palette_magma(level),
            PaletteType::Viridis => palette_viridis(level),
            PaletteType::Inferno => palette_inferno(level),
            PaletteType::Turbo => palette_turbo(level),
            PaletteType::Grayscale => {
                let v = (level * 255.0).round() as u8;
                [v, v, v]
            }
        }
    }
}

/// Original Dan Bruton's spectral algorithm as modified in Spek
fn palette_classic(mut level: f32) -> [u8; 3] {
    level *= 0.6625;
    let mut r = 0.0f32;
    let mut g = 0.0f32;
    let mut b = 0.0f32;

    if level >= 0.0 && level < 0.15 {
        r = (0.15 - level) / (0.15 + 0.075);
        g = 0.0;
        b = 1.0;
    } else if (0.15..0.275).contains(&level) {
        r = 0.0;
        g = (level - 0.15) / (0.275 - 0.15);
        b = 1.0;
    } else if (0.275..0.325).contains(&level) {
        r = 0.0;
        g = 1.0;
        b = (0.325 - level) / (0.325 - 0.275);
    } else if (0.325..0.5).contains(&level) {
        r = (level - 0.325) / (0.5 - 0.325);
        g = 1.0;
        b = 0.0;
    } else if level >= 0.5 && level <= 0.6625 {
        r = 1.0;
        g = (0.6625 - level) / (0.6625 - 0.5);
        b = 0.0;
    }

    let mut cf = 1.0f32;
    if level >= 0.0 && level < 0.1 {
        cf = level / 0.1;
    }
    cf *= 255.0;

    let rr = (r * cf).round().clamp(0.0, 255.0) as u8;
    let gg = (g * cf).round().clamp(0.0, 255.0) as u8;
    let bb = (b * cf).round().clamp(0.0, 255.0) as u8;
    [rr, gg, bb]
}

/// SoX default palette written by Rob Sykes
fn palette_sox(level: f32) -> [u8; 3] {
    let r = if (0.13..0.73).contains(&level) {
        ((level - 0.13) / 0.60 * PI / 2.0).sin()
    } else if level >= 0.73 {
        1.0
    } else {
        0.0
    };

    let g = if (0.60..0.91).contains(&level) {
        ((level - 0.60) / 0.31 * PI / 2.0).sin()
    } else if level >= 0.91 {
        1.0
    } else {
        0.0
    };

    let b = if level < 0.60 {
        0.5 * (level / 0.60 * PI).sin()
    } else if level >= 0.78 {
        (level - 0.78) / 0.22
    } else {
        0.0
    };

    let rr = (r * 255.0).round().clamp(0.0, 255.0) as u8;
    let gg = (g * 255.0).round().clamp(0.0, 255.0) as u8;
    let bb = (b * 255.0).round().clamp(0.0, 255.0) as u8;
    [rr, gg, bb]
}

/// Magma colormap polynomial approximation
fn palette_magma(t: f32) -> [u8; 3] {
    let r = (-0.002 + t * (1.785 + t * (-3.228 + t * (4.258 - t * 1.815)))).clamp(0.0, 1.0);
    let g = (-0.003 + t * (0.187 + t * (0.835 + t * (-0.158 + t * 0.136)))).clamp(0.0, 1.0);
    let b = (-0.001 + t * (1.936 + t * (-3.364 + t * (1.096 + t * 1.332)))).clamp(0.0, 1.0);
    [
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8,
    ]
}

/// Viridis colormap approximation
fn palette_viridis(t: f32) -> [u8; 3] {
    let r = (0.280 + t * (-0.140 + t * (1.800 + t * (-1.940)))).clamp(0.0, 1.0);
    let g = (0.003 + t * (1.404 + t * (-0.480 + t * 0.070))).clamp(0.0, 1.0);
    let b = (0.330 + t * (1.200 + t * (-3.200 + t * 2.670))).clamp(0.0, 1.0);
    [
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8,
    ]
}

/// Inferno colormap approximation
fn palette_inferno(t: f32) -> [u8; 3] {
    let r = (-0.001 + t * (1.611 + t * (-1.777 + t * 1.167))).clamp(0.0, 1.0);
    let g = (-0.002 + t * (0.252 + t * (0.415 + t * 0.334))).clamp(0.0, 1.0);
    let b = (0.004 + t * (1.189 + t * (-2.682 + t * 1.488))).clamp(0.0, 1.0);
    [
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8,
    ]
}

/// Google Turbo colormap approximation
fn palette_turbo(x: f32) -> [u8; 3] {
    let r = (0.1357 + x * (4.5974 + x * (-42.3278 + x * (130.5888 + x * (-150.5668 + x * 58.7047))))).clamp(0.0, 1.0);
    let g = (0.0914 + x * (2.1856 + x * (4.8052 + x * (-14.0195 + x * (4.2109 + x * 4.4347))))).clamp(0.0, 1.0);
    let b = (0.1067 + x * (12.5732 + x * (-83.0771 + x * (211.8542 + x * (-236.5656 + x * 96.0886))))).clamp(0.0, 1.0);
    [
        (r * 255.0).round() as u8,
        (g * 255.0).round() as u8,
        (b * 255.0).round() as u8,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_palettes_valid_colors() {
        for &palette in PaletteType::ALL {
            for step in 0..=100 {
                let level = step as f32 / 100.0;
                let [r, g, b] = palette.sample(level);
                assert!((0..=255).contains(&r));
                assert!((0..=255).contains(&g));
                assert!((0..=255).contains(&b));
            }
        }
    }

    #[test]
    fn test_grayscale_boundary() {
        let black = PaletteType::Grayscale.sample(0.0);
        let white = PaletteType::Grayscale.sample(1.0);
        assert_eq!(black, [0, 0, 0]);
        assert_eq!(white, [255, 255, 255]);
    }
}
