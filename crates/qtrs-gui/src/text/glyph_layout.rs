use std::str::FromStr;
use crate::text::font::Font;

/// A single positioned glyph (`QGlyphRun` item equivalent).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PositionedGlyph {
    /// Glyph index in font.
    pub glyph_id: u16,
    /// Horizontal offset in points.
    pub x: f32,
    /// Vertical offset in points.
    pub y: f32,
}

/// Shaped text layout result (`QGlyphLayout` / `QTextLayout` equivalent).
#[derive(Debug, Clone, PartialEq)]
pub struct GlyphLayout {
    /// List of ordered glyphs and their relative positions.
    pub glyphs: Vec<PositionedGlyph>,
    /// Total advance width of shaped text.
    pub width: f32,
}

impl GlyphLayout {
    /// Creates an empty layout.
    pub fn empty() -> Self {
        Self {
            glyphs: Vec::new(),
            width: 0.0,
        }
    }

    /// Shapes and positions text using `rustybuzz` with fallback to `fontdue`.
    ///
    /// - If `font.tabular_numbers` is enabled, applies OpenType `"tnum"` feature.
    /// - Applies kerning and accurate glyph advance positioning.
    /// - Falls back to fontdue if binary font data is unavailable.
    pub fn shape(text: &str, font: &Font, font_face: &fontdue::Font) -> Self {
        if text.is_empty() {
            return Self::empty();
        }

        if let Some(data) = &font.font_data {
            if let Some(rb_face) = rustybuzz::Face::from_slice(data, 0) {
                return Self::shape_with_rustybuzz(text, font, &rb_face);
            }
        }

        Self::shape_with_fontdue(text, font, font_face)
    }

    /// Shapes text using rustybuzz (supporting tnum, kerning, and OpenType features).
    pub fn shape_with_rustybuzz(text: &str, font: &Font, rb_face: &rustybuzz::Face) -> Self {
        let mut buffer = rustybuzz::UnicodeBuffer::new();
        buffer.push_str(text);

        let mut features = Vec::new();
        if font.tabular_numbers {
            if let Ok(f) = rustybuzz::Feature::from_str("tnum") {
                features.push(f);
            }
        }
        if let Ok(f) = rustybuzz::Feature::from_str("kern") {
            features.push(f);
        }

        let glyph_buffer = rustybuzz::shape(rb_face, &features, buffer);

        let upem = rb_face.units_per_em() as f32;
        let scale = if upem > 0.0 { font.size / upem } else { 1.0 };

        let infos = glyph_buffer.glyph_infos();
        let positions = glyph_buffer.glyph_positions();

        let mut current_x = 0.0;
        let mut current_y = 0.0;
        let mut glyphs = Vec::with_capacity(infos.len());

        for (info, pos) in infos.iter().zip(positions.iter()) {
            let x = current_x + (pos.x_offset as f32) * scale;
            let y = current_y + (pos.y_offset as f32) * scale;

            glyphs.push(PositionedGlyph {
                glyph_id: info.glyph_id as u16,
                x,
                y,
            });

            current_x += (pos.x_advance as f32) * scale;
            current_y += (pos.y_advance as f32) * scale;
        }

        Self {
            glyphs,
            width: current_x,
        }
    }

    /// Fallback shaping using fontdue per-character metrics.
    pub fn shape_with_fontdue(text: &str, font: &Font, font_face: &fontdue::Font) -> Self {
        let mut current_x = 0.0;
        let mut glyphs = Vec::with_capacity(text.len());

        let tnum_width = if font.tabular_numbers {
            Some(
                font_face
                    .metrics('0', font.size)
                    .advance_width
                    .max(font_face.metrics('8', font.size).advance_width),
            )
        } else {
            None
        };

        for ch in text.chars() {
            let glyph_id = font_face.lookup_glyph_index(ch);
            let metrics = font_face.metrics(ch, font.size);

            let adv_x = if ch.is_ascii_digit() {
                tnum_width.unwrap_or(metrics.advance_width)
            } else {
                metrics.advance_width
            };

            glyphs.push(PositionedGlyph {
                glyph_id,
                x: current_x,
                y: 0.0,
            });

            current_x += adv_x;
        }

        Self {
            glyphs,
            width: current_x,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    fn get_test_font() -> (Arc<Vec<u8>>, fontdue::Font) {
        let path = std::path::Path::new("C:/Windows/Fonts/arial.ttf");
        let data = if path.exists() {
            std::fs::read(path).unwrap()
        } else {
            let p2 = std::path::Path::new("C:/Windows/Fonts/segoeui.ttf");
            if p2.exists() {
                std::fs::read(p2).unwrap()
            } else {
                vec![]
            }
        };

        let font = fontdue::Font::from_bytes(data.clone(), fontdue::FontSettings::default()).unwrap();
        (Arc::new(data), font)
    }

    #[test]
    fn test_glyph_layout_fontdue_shaping() {
        let (data, font_face) = get_test_font();
        let font = Font::new("Arial", 16.0).with_font_data(data);

        let layout = GlyphLayout::shape("HUD 100%", &font, &font_face);
        assert!(!layout.glyphs.is_empty());
        assert!(layout.width > 0.0);
        assert_eq!(layout.glyphs[0].x, 0.0);
        assert!(layout.glyphs[1].x > 0.0);
    }

    #[test]
    fn test_glyph_layout_rustybuzz_tnum() {
        let (data, font_face) = get_test_font();
        let font_tnum = Font::new("Arial", 16.0)
            .with_tabular_numbers(true)
            .with_font_data(data);

        let layout_1111 = GlyphLayout::shape("1111", &font_tnum, &font_face);
        let layout_8888 = GlyphLayout::shape("8888", &font_tnum, &font_face);

        assert_eq!(layout_1111.glyphs.len(), 4);
        assert_eq!(layout_8888.glyphs.len(), 4);

        assert!((layout_1111.width - layout_8888.width).abs() < 1.0);
    }
}
