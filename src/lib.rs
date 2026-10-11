#![doc = include_str!("../README.md")]

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
