//! Styled QR code rendering with exact anti-aliasing.
//!
//! ```
//! use qrcoder::{Composer, EcLevel, Layout, Logo, ModuleSize, QuietZone, Symbol};
//!
//! let symbol = Symbol::encode(b"https://bipa.app", EcLevel::H)?;
//! let layout = Layout::new(ModuleSize::new(12)?, QuietZone::new(4)?);
//! let logo = Logo::new(&image::RgbaImage::from_pixel(64, 64, image::Rgba([0, 0, 0, 255])))?;
//! let scene = Composer::new(&symbol, layout).logo(&logo).compose()?;
//! let png = scene.to_image();
//! let svg = scene.to_svg()?;
//! assert_eq!(png.width(), (u32::from(symbol.width()) + 8) * 12);
//! assert!(svg.starts_with("<svg"));
//! # Ok::<(), qrcoder::Error>(())
//! ```

use drawing::draw_qrcode_eye;
use image::{DynamicImage, GenericImageView, Rgba, RgbaImage};
use qrcode::types::QrError;
use std::num::TryFromIntError;
use thiserror::Error;

mod budget;
mod drawing;
mod error;
mod geometry;
mod logo;
mod raster;
mod scene;
mod style;
mod svg;
mod symbol;
mod units;

pub use budget::BudgetUsage;
pub use error::Error;
pub use geometry::{Disc, Point, Rect, Ring, RoundedRect, Shape};
pub use logo::{Clearance, Logo, LogoSize, OverBudget};
pub use scene::{Composer, Layer, Layout, Scene};
pub use style::{AlignmentStyle, Color, MIN_CONTRAST, ModuleShape, Palette, PatternShape, Style};
pub use symbol::{Corner, EcLevel, Mask, Module, Pos, Role, Symbol, Version};
pub use units::{
    MAX_CANVAS_PX, MAX_MODULE_PX, MAX_PADDING, MAX_QUIET_ZONE, ModuleSize, Padding, QuietZone,
    Ratio, Roundness,
};

const RGBA_BLACK: Rgba<u8> = Rgba([0, 0, 0, 255]);
const RGBA_WHITE: Rgba<u8> = Rgba([255, 255, 255, 255]);

pub struct Builder<'a, I> {
    data: &'a [u8],
    logo_image: &'a I,
    module_width: Option<usize>,
    quiet_zone: Option<usize>,
    logo_quiet_zone: Option<usize>,
}

#[derive(Debug, Error)]
pub enum BuildError {
    #[error("Failed to encode data: {0}")]
    Encoding(QrError),
    #[error("Failed to construct codewords: {0}")]
    Codewords(QrError),
    #[error("Failed to fit the image in the drawable pixel range: {0}")]
    Geometry(#[from] TryFromIntError),
}

impl<'a, I: GenericImageView<Pixel = <RgbaImage as GenericImageView>::Pixel>> Builder<'a, I> {
    pub fn new(data: &'a [u8], logo_image: &'a I) -> Self {
        Self {
            data,
            logo_image,
            module_width: None,
            quiet_zone: None,
            logo_quiet_zone: None,
        }
    }

    #[must_use]
    pub fn with_module_width(self, module_width: usize) -> Self {
        Self {
            module_width: Some(module_width),
            ..self
        }
    }

    #[must_use]
    pub fn with_quiet_zone(self, quiet_zone: usize) -> Self {
        Self {
            quiet_zone: Some(quiet_zone),
            ..self
        }
    }

    #[must_use]
    pub fn with_logo_quiet_zone(self, logo_quiet_zone: usize) -> Self {
        Self {
            logo_quiet_zone: Some(logo_quiet_zone),
            ..self
        }
    }

    pub fn build(self) -> Result<DynamicImage, BuildError> {
        let ec_level = qrcode::EcLevel::H;
        let encoded_data =
            qrcode::bits::encode_auto(self.data, ec_level).map_err(BuildError::Encoding)?;

        let version = encoded_data.version();

        let (data_codewords, ec_codewords) =
            qrcode::ec::construct_codewords(&encoded_data.into_bytes(), version, ec_level)
                .map_err(BuildError::Codewords)?;

        let mut canvas = qrcode::canvas::Canvas::new(version, ec_level);
        canvas.draw_all_functional_patterns();
        canvas.draw_data(&data_codewords, &ec_codewords);
        let data_canvas = canvas.apply_best_mask();

        let code_width = usize::try_from(version.width())?; // code width in modules
        let module_width = self.module_width.unwrap_or(11); // module width in pixels
        let quiet_zone = self.quiet_zone.unwrap_or(4); // quiet zone width in modules

        let colors = data_canvas.into_colors();

        // Find eyes positions and dimensions
        let mut px1 = 0;
        for (x, color) in colors.iter().enumerate().take(code_width) {
            if let qrcode::Color::Light = color {
                break;
            }

            px1 = x;
        }

        let mut px2 = 0;
        for x in (0..code_width).rev() {
            if let qrcode::Color::Light = colors[x] {
                break;
            }

            px2 = x;
        }

        let mut py1 = 0;
        for y in 0..code_width {
            if let qrcode::Color::Light = colors[y * code_width] {
                break;
            }

            py1 = y;
        }

        let mut py2 = 0;
        for y in (0..code_width).rev() {
            if let qrcode::Color::Light = colors[y * code_width] {
                break;
            }

            py2 = y;
        }

        let qrcode_size = (code_width + 2 * quiet_zone) * module_width; // qrcode width in pixels
        let qrcode_data_size = code_width * module_width; // qrcode data size in pixels
        let eye_size = (px1 + 1) * module_width; // eye size in pixels
        let dot_radius = i32::try_from(module_width / 2 - module_width / 10)?; // dots size in pixels
        let margin = i32::try_from(quiet_zone * module_width)?; // quiet zone width in pixels

        let logo_img_size =
            ((qrcode_data_size * qrcode_data_size - 3 * eye_size * eye_size) * 5 / 100).isqrt(); // logo image width in pixels
        let logo_img_pos = ((code_width + 2 * quiet_zone) * module_width / 2) - logo_img_size / 2; // logo position in pixels
        let logo_quiet_zone = self.logo_quiet_zone.unwrap_or(1);

        let logo_width = u32::try_from(logo_img_size)?;

        let resized_logo = image::imageops::resize(
            self.logo_image,
            logo_width,
            logo_width,
            image::imageops::FilterType::Gaussian,
        );

        let img_width = u32::try_from(qrcode_size)?;
        let mut img = RgbaImage::from_pixel(img_width, img_width, RGBA_WHITE);

        let eye_width = u32::try_from(eye_size)?;

        // top-left eye
        draw_qrcode_eye(
            &mut img,
            (margin, margin),
            (eye_width, eye_width),
            RGBA_BLACK,
            RGBA_WHITE,
        )?;
        // top-right eye
        draw_qrcode_eye(
            &mut img,
            (i32::try_from((px2 + quiet_zone) * module_width)?, margin),
            (eye_width, eye_width),
            RGBA_BLACK,
            RGBA_WHITE,
        )?;
        // bottom-left eye
        draw_qrcode_eye(
            &mut img,
            (margin, i32::try_from((py2 + quiet_zone) * module_width)?),
            (eye_width, eye_width),
            RGBA_BLACK,
            RGBA_WHITE,
        )?;

        for y in 0..code_width {
            for x in 0..code_width {
                let c = colors[y * code_width + x];

                if let qrcode::Color::Dark = c
                    && !((y >= py2 || y <= py1) && x <= px1 || x >= px2 && y <= py1)
                {
                    let cx = module_width / 2 + (x + quiet_zone) * module_width;
                    let cy = module_width / 2 + (y + quiet_zone) * module_width;

                    if cx >= (logo_img_pos - logo_quiet_zone * module_width)
                        && cx <= (logo_img_pos + logo_img_size + logo_quiet_zone * module_width)
                        && cy >= (logo_img_pos - logo_quiet_zone * module_width)
                        && cy <= (logo_img_pos + logo_img_size + logo_quiet_zone * module_width)
                    {
                        continue;
                    }

                    imageproc::drawing::draw_filled_circle_mut(
                        &mut img,
                        (i32::try_from(cx)?, i32::try_from(cy)?),
                        dot_radius,
                        RGBA_BLACK,
                    );
                }
            }
        }

        let logo_origin = i64::try_from(logo_img_pos)?;
        image::imageops::overlay(&mut img, &resized_logo, logo_origin, logo_origin);

        Ok(DynamicImage::ImageRgba8(img))
    }
}

#[cfg(test)]
mod tests {
    use image::{DynamicImage, Rgba, RgbaImage};

    #[test]
    fn it_works() -> Result<(), Box<dyn std::error::Error>> {
        let logo =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(512, 512, Rgba([255, 0, 0, 255])));

        let img = super::Builder::new(b"12345678abcdefghijklmnopqrstuvwxyz".as_ref(), &logo)
            .with_module_width(44)
            .build()?;

        let img_gray = img.into_luma8();

        let mut decoder = quircs::Quirc::default();

        let codes = decoder
            .identify(
                usize::try_from(img_gray.width()).unwrap(),
                usize::try_from(img_gray.height()).unwrap(),
                &img_gray,
            )
            .collect::<Result<Vec<_>, _>>()?;
        assert_eq!(codes.len(), 1);

        let code = codes[0];
        let decoded_data = code.decode()?;

        assert_eq!(
            std::str::from_utf8(&decoded_data.payload)?,
            "12345678abcdefghijklmnopqrstuvwxyz"
        );
        Ok(())
    }

    #[test]
    fn refuses_a_quiet_zone_wider_than_the_drawable_range() {
        let logo =
            DynamicImage::ImageRgba8(RgbaImage::from_pixel(512, 512, Rgba([255, 0, 0, 255])));

        let err = super::Builder::new(b"12345678".as_ref(), &logo)
            .with_quiet_zone(usize::try_from(i32::MAX).unwrap())
            .build()
            .unwrap_err();

        assert!(matches!(err, super::BuildError::Geometry(_)));
    }
}
