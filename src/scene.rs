use std::collections::BTreeSet;
use std::io::Cursor;

use image::{DynamicImage, ImageFormat, RgbaImage};

use crate::budget::BudgetUsage;
use crate::error::Error;
use crate::geometry::{Point, Rect, Shape};
use crate::logo::{Logo, PlacedLogo};
use crate::style::{AlignmentStyle, Blending, Color, Neighbors, Side, Style};
use crate::symbol::{Pos, Role, Symbol};
use crate::units::{MAX_CANVAS_PX, MAX_MODULE_PX, ModuleSize, QuietZone};
use crate::{raster, svg};

const FINDER_SPAN: u8 = 7;
const ALIGNMENT_SPAN: u8 = 5;
const FORMAT_SPAN: u8 = FINDER_SPAN + 2;
const CORRECTABLE_INFO_BITS: u8 = 3;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Layout {
    pub module_size: ModuleSize,
    pub quiet_zone: QuietZone,
}

impl Layout {
    #[must_use]
    pub const fn new(module_size: ModuleSize, quiet_zone: QuietZone) -> Self {
        Self {
            module_size,
            quiet_zone,
        }
    }

    pub fn fit(symbol: &Symbol, max_px: u32, quiet_zone: QuietZone) -> Result<Self, Error> {
        let modules = u32::from(symbol.width()) + 2 * u32::from(quiet_zone.modules());
        let module_size = ModuleSize::new((max_px / modules).min(MAX_MODULE_PX))?;
        Ok(Self::new(module_size, quiet_zone))
    }

    pub fn canvas_px(&self, symbol: &Symbol) -> Result<u32, Error> {
        let modules = u64::from(symbol.width()) + 2 * u64::from(self.quiet_zone.modules());
        let px = modules * u64::from(self.module_size.px());
        u32::try_from(px)
            .ok()
            .filter(|px| *px <= MAX_CANVAS_PX)
            .ok_or(Error::CanvasTooLarge {
                px,
                max: MAX_CANVAS_PX,
            })
    }

    #[must_use]
    pub fn cell(&self, pos: Pos) -> Rect {
        self.area(pos, 1)
    }

    #[must_use]
    pub fn area(&self, origin: Pos, span: u8) -> Rect {
        let m = f64::from(self.module_size.px());
        let q = f64::from(self.quiet_zone.modules());
        Rect::from_origin(
            Point::new((q + f64::from(origin.x)) * m, (q + f64::from(origin.y)) * m),
            f64::from(span) * m,
            f64::from(span) * m,
        )
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    pub color: Color,
    pub shapes: Vec<Shape>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    size: u32,
    background: Color,
    blending: Blending,
    layers: Vec<Layer>,
    logo: Option<PlacedLogo>,
    budget: BudgetUsage,
}

impl Scene {
    #[must_use]
    pub const fn size(&self) -> u32 {
        self.size
    }

    #[must_use]
    pub const fn background(&self) -> Color {
        self.background
    }

    #[must_use]
    pub const fn blending(&self) -> Blending {
        self.blending
    }

    #[must_use]
    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    #[must_use]
    pub const fn error_budget_usage(&self) -> BudgetUsage {
        self.budget
    }

    #[must_use]
    pub fn logo_rect(&self) -> Option<Rect> {
        self.logo.as_ref().map(PlacedLogo::rect)
    }

    pub fn push_layer(&mut self, layer: Layer) {
        self.layers.push(layer);
    }

    pub(crate) const fn logo(&self) -> Option<&PlacedLogo> {
        self.logo.as_ref()
    }

    #[must_use]
    pub fn to_image(&self) -> RgbaImage {
        raster::rasterize(self)
    }

    pub fn to_svg(&self) -> Result<String, Error> {
        svg::render(self)
    }

    pub fn to_png(&self) -> Result<Vec<u8>, Error> {
        let mut png = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(self.to_image()).write_to(&mut png, ImageFormat::Png)?;
        Ok(png.into_inner())
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Composer<'a> {
    symbol: &'a Symbol,
    layout: Layout,
    style: Style,
    logo: Option<&'a Logo>,
}

impl<'a> Composer<'a> {
    #[must_use]
    pub fn new(symbol: &'a Symbol, layout: Layout) -> Self {
        Self {
            symbol,
            layout,
            style: Style::default(),
            logo: None,
        }
    }

    #[must_use]
    pub const fn style(self, style: Style) -> Self {
        Self { style, ..self }
    }

    #[must_use]
    pub const fn logo(self, logo: &'a Logo) -> Self {
        Self {
            logo: Some(logo),
            ..self
        }
    }

    pub fn compose(&self) -> Result<Scene, Error> {
        let size = self.layout.canvas_px(self.symbol)?;
        let min = self.style.module.min_module_px();
        let px = self.layout.module_size.px();
        if px < min {
            return Err(Error::ModuleTooSmall { px, min });
        }
        let fallbacks = match self.logo {
            Some(logo) => logo.over_budget().fallback_scales(),
            None => &[],
        };
        fallbacks
            .iter()
            .fold(self.compose_at(size, 1.0), |scene, scale| {
                scene.or_else(|_| self.compose_at(size, *scale))
            })
    }

    fn compose_at(&self, size: u32, logo_scale: f64) -> Result<Scene, Error> {
        let placed = self.place_logo(size, logo_scale)?;
        let hidden = self.hidden_modules(placed.as_ref());
        let misread = self.misread_modules(&hidden, placed.as_ref());
        check_function_patterns(self.symbol, &misread)?;
        let budget = BudgetUsage::of(self.symbol, misread);
        if let Some(logo) = self.logo {
            check_budget(budget, logo)?;
        }
        Ok(Scene {
            size,
            background: self.style.palette.light(),
            blending: self.style.blending,
            layers: self.layers(&hidden)?,
            logo: placed,
            budget,
        })
    }

    fn layers(&self, hidden: &BTreeSet<Pos>) -> Result<Vec<Layer>, Error> {
        let palette = self.style.palette;
        let finder = Layer {
            color: palette.finder(),
            shapes: self.finder_shapes()?,
        };
        let (patterned, alignment) = match self.style.alignment {
            AlignmentStyle::Pattern(pattern) => {
                let origins: Vec<Pos> = self.visible_alignments(hidden).collect();
                let mut shapes = Vec::new();
                for origin in &origins {
                    shapes.extend(pattern.shapes_in(self.layout.area(*origin, ALIGNMENT_SPAN))?);
                }
                let patterned = origins
                    .iter()
                    .flat_map(|o| span(*o, ALIGNMENT_SPAN))
                    .collect();
                let layer = Layer {
                    color: palette.dark(),
                    shapes,
                };
                (patterned, Some(layer))
            }
            AlignmentStyle::Modules => (BTreeSet::new(), None),
        };
        let modules = Layer {
            color: palette.dark(),
            shapes: self.module_shapes(hidden, &patterned),
        };
        let mut layers = vec![modules, finder];
        layers.extend(alignment);
        Ok(layers)
    }

    fn misread_modules(&self, hidden: &BTreeSet<Pos>, placed: Option<&PlacedLogo>) -> Vec<Pos> {
        let background = self.style.palette.light();
        hidden
            .iter()
            .copied()
            .filter(|pos| {
                let reads_dark =
                    placed.is_some_and(|logo| logo.reads_dark(self.layout.cell(*pos), background));
                self.symbol
                    .module(*pos)
                    .is_some_and(|m| m.dark != reads_dark)
            })
            .collect()
    }

    fn place_logo(&self, size: u32, scale: f64) -> Result<Option<PlacedLogo>, Error> {
        let m = f64::from(self.layout.module_size.px());
        let symbol_px = f64::from(self.symbol.width()) * m;
        self.logo
            .map(|logo| logo.place(size, symbol_px, f64::from(FINDER_SPAN) * m, scale))
            .transpose()
    }

    fn hidden_modules(&self, placed: Option<&PlacedLogo>) -> BTreeSet<Pos> {
        let (Some(placed), Some(logo)) = (placed, self.logo) else {
            return BTreeSet::new();
        };
        let m = f64::from(self.layout.module_size.px());
        let extent = self.module_extent();
        let clearance = (f64::from(logo.padding().modules()) * m).max(extent);
        let reach = placed.rect();
        self.symbol
            .modules()
            .map(|(pos, _)| pos)
            .filter(|pos| {
                let center = self.layout.cell(*pos).center();
                reach.distance_to(center) <= clearance && placed.distance(center) <= clearance
            })
            .collect()
    }

    fn module_extent(&self) -> f64 {
        let cell = self.layout.cell(Pos::new(0, 0));
        let center = cell.center();
        self.style.module.shape_in(cell).map_or(0.0, |shape| {
            let b = shape.bounds();
            match shape {
                Shape::Disc(disc) => disc.radius(),
                Shape::Rect(_) | Shape::RoundedRect(_) | Shape::Ring(_) | Shape::Tile(_) => {
                    center.distance(b.max())
                }
            }
        })
    }

    fn module_shapes(&self, hidden: &BTreeSet<Pos>, patterned: &BTreeSet<Pos>) -> Vec<Shape> {
        let drawn = |x: Option<u8>, y: Option<u8>| {
            x.zip(y).map(|(x, y)| Pos::new(x, y)).is_some_and(|pos| {
                self.symbol.module(pos).is_some_and(|module| {
                    let own_shape = match module.role {
                        Role::Finder => true,
                        Role::Alignment => patterned.contains(&pos),
                        Role::Separator
                        | Role::Timing
                        | Role::FormatInfo
                        | Role::VersionInfo
                        | Role::Data => false,
                    };
                    module.dark && !own_shape && !hidden.contains(&pos)
                })
            })
        };
        self.symbol
            .modules()
            .filter(|(pos, _)| drawn(Some(pos.x), Some(pos.y)))
            .filter_map(|(pos, _)| {
                let around = [
                    (Side::Up, drawn(Some(pos.x), pos.y.checked_sub(1))),
                    (Side::Right, drawn(pos.x.checked_add(1), Some(pos.y))),
                    (Side::Down, drawn(Some(pos.x), pos.y.checked_add(1))),
                    (Side::Left, drawn(pos.x.checked_sub(1), Some(pos.y))),
                ];
                let neighbors = around
                    .into_iter()
                    .filter(|(_, present)| *present)
                    .fold(Neighbors::NONE, |n, (side, _)| n.with(side));
                self.style
                    .module
                    .shape_with(self.layout.cell(pos), neighbors)
            })
            .collect()
    }

    fn finder_shapes(&self) -> Result<Vec<Shape>, Error> {
        let mut shapes = Vec::new();
        for (_, origin) in self.symbol.finders() {
            shapes.extend(
                self.style
                    .finder
                    .shapes_in(self.layout.area(origin, FINDER_SPAN))?,
            );
        }
        Ok(shapes)
    }

    fn visible_alignments<'b>(
        &'b self,
        hidden: &'b BTreeSet<Pos>,
    ) -> impl Iterator<Item = Pos> + 'b {
        self.symbol
            .alignment_centers()
            .into_iter()
            .map(|c| Pos::new(c.x - ALIGNMENT_SPAN / 2, c.y - ALIGNMENT_SPAN / 2))
            .filter(move |origin| !span(*origin, ALIGNMENT_SPAN).any(|pos| hidden.contains(&pos)))
    }
}

fn span(origin: Pos, side: u8) -> impl Iterator<Item = Pos> {
    (origin.y..origin.y + side)
        .flat_map(move |y| (origin.x..origin.x + side).map(move |x| Pos::new(x, y)))
}

fn check_function_patterns(symbol: &Symbol, misread: &[Pos]) -> Result<(), Error> {
    let mut format_errors = [0u8; 2];
    let mut version_errors = [0u8; 2];
    for pos in misread {
        let Some(module) = symbol.module(*pos) else {
            continue;
        };
        let errors = match module.role {
            Role::Data | Role::Alignment => continue,
            Role::Finder | Role::Separator | Role::Timing => {
                return Err(Error::LogoHidesFunctionPattern(module.role));
            }
            Role::FormatInfo => &mut format_errors[usize::from(pos.x.max(pos.y) >= FORMAT_SPAN)],
            Role::VersionInfo => &mut version_errors[usize::from(pos.y < pos.x)],
        };
        *errors = errors.saturating_add(1);
    }
    for (role, [first, second]) in [
        (Role::FormatInfo, format_errors),
        (Role::VersionInfo, version_errors),
    ] {
        if first.max(second) > CORRECTABLE_INFO_BITS || (first > 0 && second > 0) {
            return Err(Error::LogoHidesFunctionPattern(role));
        }
    }
    Ok(())
}

fn check_budget(budget: BudgetUsage, logo: &Logo) -> Result<(), Error> {
    if budget.fraction() <= logo.error_budget().get() {
        Ok(())
    } else {
        Err(Error::LogoExceedsErrorBudget {
            used: budget.worst_block(),
            budget: budget.correctable(),
        })
    }
}

#[cfg(test)]
mod tests {
    use image::Rgba;

    use super::*;
    use crate::logo::{Clearance, LogoSize, OverBudget};
    use crate::style::PatternShape;
    use crate::symbol::{EcLevel, Version};
    use crate::units::{Padding, Ratio, Roundness};

    fn square_logo() -> Logo {
        Logo::new(&RgbaImage::from_pixel(8, 8, Rgba([0, 0, 0, 255]))).expect("logo")
    }

    fn layout(px: u32, quiet: u32) -> Layout {
        Layout::new(
            ModuleSize::new(px).expect("module size"),
            QuietZone::new(quiet).expect("quiet zone"),
        )
    }

    #[test]
    fn square_logos_fit_every_version_when_every_hidden_module_is_misread() {
        for (px, quiet, padding) in [(11, 2, 2), (22, 4, 1)] {
            let logo = square_logo()
                .with_clearance(Clearance::Bounds)
                .with_padding(Padding::new(padding).expect("padding"));
            let layout = layout(px, quiet);
            for number in 1..=40 {
                let version = Version::new(number).expect("version");
                let symbol = Symbol::encode_with(b"x", EcLevel::H, version).expect("encode");
                let composer = Composer::new(&symbol, layout).logo(&logo);
                let size = layout.canvas_px(&symbol).expect("canvas");
                let fits = |scale: f64| {
                    let placed = composer.place_logo(size, scale).expect("placed");
                    let hidden: Vec<Pos> = composer
                        .hidden_modules(placed.as_ref())
                        .into_iter()
                        .collect();
                    check_function_patterns(&symbol, &hidden).is_ok()
                        && BudgetUsage::of(&symbol, hidden).fraction() <= logo.error_budget().get()
                };
                let scales = std::iter::once(&1.0).chain(logo.over_budget().fallback_scales());
                assert!(scales.copied().any(fits), "{px}px v{number}");
            }
        }
    }

    #[test]
    fn logos_that_hide_function_patterns_are_refused() {
        let symbol = Symbol::encode(b"x", EcLevel::H).expect("encode");
        let logo = square_logo()
            .with_size(LogoSize::Width(Ratio::new(0.9).expect("ratio")))
            .with_over_budget(OverBudget::Reject);
        let scene = Composer::new(&symbol, layout(10, 4)).logo(&logo).compose();
        assert!(matches!(scene, Err(Error::LogoHidesFunctionPattern(_))));
    }

    #[test]
    fn information_copies_accept_only_correctable_damage_in_one_copy() {
        let symbol = Symbol::encode(b"x", EcLevel::H).expect("encode");
        let info: Vec<Pos> = symbol
            .modules()
            .filter(|(_, module)| module.role == Role::FormatInfo)
            .map(|(pos, _)| pos)
            .collect();
        let (near, far): (Vec<Pos>, Vec<Pos>) =
            info.iter().partition(|pos| pos.x.max(pos.y) < FORMAT_SPAN);
        assert_eq!((near.len(), far.len()), (15, 16));
        assert!(check_function_patterns(&symbol, &near[..3]).is_ok());
        assert!(check_function_patterns(&symbol, &far[..3]).is_ok());
        for misread in [&near[..4], &far[..4], &[near[0], far[0]]] {
            assert!(matches!(
                check_function_patterns(&symbol, misread),
                Err(Error::LogoHidesFunctionPattern(Role::FormatInfo))
            ));
        }
        let version = Symbol::encode_with(b"x", EcLevel::H, Version::new(7).expect("version"))
            .expect("encode");
        let (right, left): (Vec<Pos>, Vec<Pos>) = version
            .modules()
            .filter(|(_, module)| module.role == Role::VersionInfo)
            .map(|(pos, _)| pos)
            .partition(|pos| pos.y < pos.x);
        assert_eq!((right.len(), left.len()), (18, 18));
        assert!(check_function_patterns(&version, &right[..3]).is_ok());
        assert!(check_function_patterns(&version, &left[..3]).is_ok());
        for misread in [&right[..4], &[right[0], left[0]]] {
            assert!(matches!(
                check_function_patterns(&version, misread),
                Err(Error::LogoHidesFunctionPattern(Role::VersionInfo))
            ));
        }
        let timing = Pos::new(10, 6);
        assert_eq!(symbol.module(timing).map(|m| m.role), Some(Role::Timing));
        assert!(matches!(
            check_function_patterns(&symbol, &[timing]),
            Err(Error::LogoHidesFunctionPattern(Role::Timing))
        ));
    }

    #[test]
    fn visible_modules_of_a_partly_hidden_alignment_pattern_are_drawn() {
        let symbol = Symbol::encode_with(b"x", EcLevel::H, Version::new(2).expect("version"))
            .expect("encode");
        let composer = Composer::new(&symbol, layout(10, 4));
        let origin = Pos::new(16, 16);
        let area = composer.layout.area(origin, ALIGNMENT_SPAN);
        let in_area = |hidden: &BTreeSet<Pos>| {
            let layers = composer.layers(hidden).expect("layers");
            layers[0]
                .shapes
                .iter()
                .filter(|shape| area.distance_to(shape.bounds().center()) <= 0.0)
                .count()
        };
        assert_eq!(in_area(&BTreeSet::new()), 0);
        let hidden = BTreeSet::from([origin]);
        let dark = span(origin, ALIGNMENT_SPAN)
            .filter(|pos| !hidden.contains(pos))
            .filter(|pos| symbol.module(*pos).is_some_and(|m| m.dark))
            .count();
        assert_eq!(dark, 16);
        assert_eq!(in_area(&hidden), dark);
    }

    #[test]
    fn finder_shapes_that_do_not_nest_are_refused() {
        let symbol = Symbol::encode(b"x", EcLevel::H).expect("encode");
        let style = Style {
            finder: PatternShape::SQUARE_FINDER.with_outer_corner(Roundness::CIRCLE),
            ..Style::BRAND
        };
        let scene = Composer::new(&symbol, layout(10, 4)).style(style).compose();
        assert!(matches!(scene, Err(Error::PatternProportions)));
    }
}
