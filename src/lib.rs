//! Styled QR code rendering with exact anti-aliasing.
//!
//! ```
//! use styled_qr::{Composer, EcLevel, Layout, Logo, ModuleSize, QuietZone, Symbol};
//!
//! let symbol = Symbol::encode(b"https://example.com", EcLevel::H)?;
//! let layout = Layout::new(ModuleSize::new(12)?, QuietZone::new(4)?);
//! let logo = Logo::new(&image::RgbaImage::from_pixel(64, 64, image::Rgba([0, 0, 0, 255])))?;
//! let scene = Composer::new(&symbol, layout).logo(&logo).compose()?;
//! let png = scene.to_image();
//! let svg = scene.to_svg()?;
//! assert_eq!(png.width(), (u32::from(symbol.width()) + 8) * 12);
//! assert!(svg.starts_with("<svg"));
//! # Ok::<(), styled_qr::Error>(())
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

pub use budget::BudgetUsage;
pub use error::Error;
pub use geometry::{Corners, Disc, Point, Rect, RectCorner, Ring, RoundedRect, Shape, Tile};
pub use logo::{Clearance, Logo, LogoSize, OverBudget, Plate};
pub use scene::{Composer, Layer, Layout, Scene};
pub use style::{
    AlignmentStyle, Blending, Color, MIN_CONTRAST, ModuleShape, Neighbors, Palette, PatternShape,
    Side, Style,
};
pub use symbol::{Corner, EcLevel, Mask, Module, Pos, Role, Symbol, Version};
pub use units::{
    MAX_CANVAS_PX, MAX_MODULE_PX, MAX_PADDING, MAX_QUIET_ZONE, ModuleSize, Padding, QuietZone,
    Ratio, Roundness,
};
