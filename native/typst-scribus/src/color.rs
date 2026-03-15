//! Color conversion utilities for Scribus export.
//!
//! Scribus supports named colors in CMYK or RGB color spaces.
//! We convert Typst [`Paint`] values into Scribus-compatible color strings.

use typst_library::visualize::{Color, Paint};

/// A Scribus color definition emitted as an inline `FCOLOR`/`SCOLOR` reference.
/// Scribus uses color names + shade percentages for fill/stroke.
///
/// For simplicity we use inline hex colors via the RGB color space, which
/// Scribus supports when defined in the COLOR palette or as direct attrs.
#[derive(Debug, Clone)]
pub struct SlaColor {
    /// Scribus color name (generated from hex value).
    pub name: String,
    /// CMYK components as percentages (0–100).
    pub c: f64,
    pub m: f64,
    pub y: f64,
    pub k: f64,
}

impl SlaColor {
    /// Convert a Typst [`Paint`] to an [`SlaColor`].
    ///
    /// Gradients and patterns are approximated by their first solid color,
    /// falling back to black.
    pub fn from_paint(paint: &Paint) -> Self {
        match paint {
            Paint::Solid(color) => Self::from_color(*color),
            // For gradients and tilings, fall back to black.
            _ => Self {
                name: String::from("Black"),
                c: 0.0,
                m: 0.0,
                y: 0.0,
                k: 100.0,
            },
        }
    }

    /// Convert a Typst [`Color`] to an [`SlaColor`] using CMYK.
    pub fn from_color(color: Color) -> Self {
        let cmyk = color.to_cmyk();
        // Cmyk fields are public f32 in range [0.0, 1.0].
        let c = (cmyk.c as f64) * 100.0;
        let m = (cmyk.m as f64) * 100.0;
        let y = (cmyk.y as f64) * 100.0;
        let k = (cmyk.k as f64) * 100.0;

        // Generate a stable name from CMYK values.
        let name = if k >= 99.9 && c < 0.1 && m < 0.1 && y < 0.1 {
            String::from("Black")
        } else if k < 0.1 && c < 0.1 && m < 0.1 && y < 0.1 {
            String::from("White")
        } else {
            format!(
                "CMYK_{:03}_{:03}_{:03}_{:03}",
                (c * 2.55) as u8,
                (m * 2.55) as u8,
                (y * 2.55) as u8,
                (k * 2.55) as u8,
            )
        };

        Self { name, c, m, y, k }
    }

    /// Get the CMYK components formatted for Scribus percentage attributes.
    pub fn c_pct(&self) -> String {
        format!("{:.4}", self.c * 2.55)
    }

    pub fn m_pct(&self) -> String {
        format!("{:.4}", self.m * 2.55)
    }

    pub fn y_pct(&self) -> String {
        format!("{:.4}", self.y * 2.55)
    }

    pub fn k_pct(&self) -> String {
        format!("{:.4}", self.k * 2.55)
    }
}
