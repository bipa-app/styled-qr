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

mod budget;
mod error;
mod geometry;
mod logo;
mod raster;
mod scene;
mod style;
mod svg;
mod symbol;
mod units;

use image::{DynamicImage, GenericImageView, Rgba};

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

pub type BuildError = Error;

const LEGACY_MODULE_PX: usize = 11;
const LEGACY_QUIET_ZONE: usize = 4;
const LEGACY_LOGO_PADDING: usize = 1;

pub struct Builder<'a, I> {
    data: &'a [u8],
    logo_image: &'a I,
    module_width: usize,
    quiet_zone: usize,
    logo_quiet_zone: usize,
}

impl<'a, I: GenericImageView<Pixel = Rgba<u8>>> Builder<'a, I> {
    pub fn new(data: &'a [u8], logo_image: &'a I) -> Self {
        Self {
            data,
            logo_image,
            module_width: LEGACY_MODULE_PX,
            quiet_zone: LEGACY_QUIET_ZONE,
            logo_quiet_zone: LEGACY_LOGO_PADDING,
        }
    }

    #[must_use]
    pub fn with_module_width(self, module_width: usize) -> Self {
        Self {
            module_width,
            ..self
        }
    }

    #[must_use]
    pub fn with_quiet_zone(self, quiet_zone: usize) -> Self {
        Self { quiet_zone, ..self }
    }

    #[must_use]
    pub fn with_logo_quiet_zone(self, logo_quiet_zone: usize) -> Self {
        Self {
            logo_quiet_zone,
            ..self
        }
    }

    pub fn build(self) -> Result<DynamicImage, Error> {
        let symbol = Symbol::encode(self.data, EcLevel::H)?;
        let layout = Layout::new(
            ModuleSize::new(saturate(self.module_width))?,
            QuietZone::new(saturate(self.quiet_zone))?,
        );
        let logo =
            Logo::new(self.logo_image)?.with_padding(Padding::new(saturate(self.logo_quiet_zone))?);
        let scene = Composer::new(&symbol, layout).logo(&logo).compose()?;
        Ok(DynamicImage::ImageRgba8(scene.to_image()))
    }
}

fn saturate(value: usize) -> u32 {
    u32::try_from(value).unwrap_or(u32::MAX)
}
