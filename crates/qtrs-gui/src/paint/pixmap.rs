//! In-memory offscreen pixmap surface (`QPixmap` / `QImage` equivalent).
//!
//! Provides pixel storage and offscreen rasterization backed by `tiny_skia::Pixmap`.

use crate::paint::paint_device::PaintDevice;
use std::path::Path;

/// Offscreen raster surface supporting HiDPI scaling and pixel buffer operations.
#[derive(Clone, Debug)]
pub struct Pixmap {
    pixmap: tiny_skia::Pixmap,
    dpr: f32,
}

impl Pixmap {
    /// Creates a pixmap with the specified physical dimensions and default DPR of 1.0.
    pub fn new(width: u32, height: u32) -> Option<Self> {
        Self::with_dpr(width, height, 1.0)
    }

    /// Creates a pixmap with DPR support.
    pub fn with_dpr(width: u32, height: u32, dpr: f32) -> Option<Self> {
        let pixmap = tiny_skia::Pixmap::new(width.max(1), height.max(1))?;
        Some(Self { pixmap, dpr })
    }
    /// Physical width in pixels.
    #[inline]
    pub fn physical_width(&self) -> u32 {
        self.pixmap.width()
    }

    /// Physical height in pixels.
    #[inline]
    pub fn physical_height(&self) -> u32 {
        self.pixmap.height()
    }

    /// Logical width considering DPR.
    #[inline]
    pub fn logical_width(&self) -> f32 {
        self.pixmap.width() as f32 / self.dpr
    }

    /// Logical height considering DPR.
    #[inline]
    pub fn logical_height(&self) -> f32 {
        self.pixmap.height() as f32 / self.dpr
    }

    /// Creates a Pixmap from a canonical CPU `Image`.
    pub fn from_image(image: &crate::image::Image) -> Option<Self> {
        if image.is_null() {
            return None;
        }
        let rgba_image = if image.format() == crate::image::ImageFormat::Rgba8888Premultiplied {
            image.clone()
        } else {
            image.converted_to(crate::image::ImageFormat::Rgba8888Premultiplied)
        };
        let mut pm = Self::with_dpr(rgba_image.width(), rgba_image.height(), rgba_image.device_pixel_ratio())?;
        pm.data_mut().copy_from_slice(rgba_image.data());
        Some(pm)
    }

    /// Converts this platform Pixmap back into a canonical CPU `Image`.
    pub fn to_image(&self) -> crate::image::Image {
        let mut img = crate::image::Image::with_dpr(
            self.pixmap.width(),
            self.pixmap.height(),
            crate::image::ImageFormat::Rgba8888Premultiplied,
            self.dpr,
        );
        img.data_mut().copy_from_slice(self.data());
        img
    }


    /// Fills the surface with the specified color.
    pub fn fill(&mut self, color: tiny_skia::Color) {
        self.pixmap.fill(color);
    }

    /// Returns the raw RGBA byte slice.
    pub fn data(&self) -> &[u8] {
        self.pixmap.data()
    }

    /// Returns the mutable RGBA byte slice.
    pub fn data_mut(&mut self) -> &mut [u8] {
        self.pixmap.data_mut()
    }
    /// Gets the color of pixel at (x, y).
    pub fn pixel(&self, x: u32, y: u32) -> Option<tiny_skia::PremultipliedColorU8> {
        self.pixmap.pixel(x, y)
    }

    /// Returns a reference to the underlying tiny_skia::Pixmap.
    pub fn as_tiny_skia(&self) -> &tiny_skia::Pixmap {
        &self.pixmap
    }

    /// Returns a mutable reference to the underlying tiny_skia::Pixmap.
    pub fn as_tiny_skia_mut(&mut self) -> &mut tiny_skia::Pixmap {
        &mut self.pixmap
    }

    /// Converts RGBA pixels to BGRA format in-place for Windows UpdateLayeredWindow.
    #[inline]
    pub fn convert_to_bgra_in_place(&mut self) {
        let data = self.pixmap.data_mut();
        for chunk in data.chunks_exact_mut(4) {
            chunk.swap(0, 2);
        }
    }

    /// Clones and converts the pixel buffer to BGRA byte vector.
    pub fn to_bgra_vec(&self) -> Vec<u8> {
        let mut bytes = self.pixmap.data().to_vec();
        for chunk in bytes.chunks_exact_mut(4) {
            chunk.swap(0, 2);
        }
        bytes
    }

    /// Saves the pixmap content to a PNG file.
    pub fn save_png(&self, path: &Path) -> Result<(), png::EncodingError> {
        self.pixmap.save_png(path)
    }
}

impl PaintDevice for Pixmap {
    fn physical_width(&self) -> u32 {
        self.pixmap.width()
    }

    fn physical_height(&self) -> u32 {
        self.pixmap.height()
    }

    fn device_pixel_ratio(&self) -> f32 {
        self.dpr
    }

    fn as_pixmap_mut(&mut self) -> tiny_skia::PixmapMut<'_> {
        self.pixmap.as_mut()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::primitives::Size;

    #[test]
    fn test_pixmap_creation_and_paint_device_impl() {
        let mut pm = Pixmap::with_dpr(200, 100, 2.0).expect("valid pixmap");

        assert_eq!(pm.physical_width(), 200);
        assert_eq!(pm.physical_height(), 100);
        assert_eq!(pm.device_pixel_ratio(), 2.0);
        assert_eq!(pm.width(), 100.0);
        assert_eq!(pm.height(), 50.0);
        assert_eq!(pm.size(), Size::new(100, 50));

        let white = tiny_skia::Color::WHITE;
        pm.fill(white);

        let data = pm.data();
        assert_eq!(data[0], 255);
        assert_eq!(data[1], 255);
        assert_eq!(data[2], 255);
        assert_eq!(data[3], 255);

        let data_mut = pm.data_mut();
        data_mut[0] = 128;
        assert_eq!(pm.data()[0], 128);

        let pixmap_mut = pm.as_pixmap_mut();
        assert_eq!(pixmap_mut.width(), 200);
        assert_eq!(pixmap_mut.height(), 100);
    }

    #[test]
    fn test_pixmap_bgra_conversion() {
        let mut pm = Pixmap::new(2, 2).expect("valid pixmap");
        let data = pm.data_mut();
        data[0] = 200; // R
        data[1] = 100; // G
        data[2] = 50;  // B
        data[3] = 128; // A

        let bgra_vec = pm.to_bgra_vec();
        assert_eq!(bgra_vec[0], 50);  // B
        assert_eq!(bgra_vec[1], 100); // G
        assert_eq!(bgra_vec[2], 200); // R
        assert_eq!(bgra_vec[3], 128); // A

        pm.convert_to_bgra_in_place();
        assert_eq!(pm.data()[0], 50);  // B
        assert_eq!(pm.data()[1], 100); // G
        assert_eq!(pm.data()[2], 200); // R
        assert_eq!(pm.data()[3], 128); // A

        pm.convert_to_bgra_in_place();
        assert_eq!(pm.data()[0], 200);
        assert_eq!(pm.data()[1], 100);
        assert_eq!(pm.data()[2], 50);
        assert_eq!(pm.data()[3], 128);
    }

    #[test]
    fn test_pixmap_allocation_and_dpr() {
        let pm = Pixmap::with_dpr(200, 100, 2.0).expect("allocation failed");
        assert_eq!(pm.physical_width(), 200);
        assert_eq!(pm.physical_height(), 100);
        assert_eq!(pm.device_pixel_ratio(), 2.0);
        assert_eq!(pm.size(), Size::new(100, 50));
    }

    #[test]
    fn test_pixmap_fill_and_raw_bytes() {
        let mut pm = Pixmap::new(2, 2).expect("allocation failed");

        let red = tiny_skia::Color::from_rgba8(255, 0, 0, 255);
        pm.fill(red);

        let data = pm.data();
        assert_eq!(data.len(), 2 * 2 * 4);
        assert_eq!(data[0], 255); // R
        assert_eq!(data[1], 0);   // G
        assert_eq!(data[2], 0);   // B
        assert_eq!(data[3], 255); // A
    }

    #[test]
    fn test_win32_bgra_conversion() {
        let mut pm = Pixmap::new(1, 1).expect("allocation failed");
        pm.fill(tiny_skia::Color::from_rgba8(255, 0, 0, 255));

        pm.convert_to_bgra_in_place();

        let data = pm.data();
        assert_eq!(data[0], 0);   // B
        assert_eq!(data[1], 0);   // G
        assert_eq!(data[2], 255); // R
        assert_eq!(data[3], 255); // A
    }

    #[test]
    fn test_save_png_visual_verification() {
        let mut pm = Pixmap::new(100, 100).unwrap();
        pm.fill(tiny_skia::Color::from_rgba8(0, 128, 255, 200));

        let out_dir = std::path::Path::new("target");
        if !out_dir.exists() {
            let _ = std::fs::create_dir_all(out_dir);
        }
        let out_path = out_dir.join("test_device_output.png");

        pm.save_png(&out_path).expect("failed to save PNG");
        assert!(out_path.exists());
    }
}
