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
        let budget = BudgetUsage::of(self.symbol, self.misread_modules(&hidden, placed.as_ref()));
        if let Some(logo) = self.logo {
            check_budget(budget, logo)?;
        }
        Ok(Scene {
            size,
            background: self.style.palette.light(),
            blending: self.style.blending,
            layers: self.layers(&hidden),
            logo: placed,
            budget,
        })
    }

    fn layers(&self, hidden: &BTreeSet<Pos>) -> Vec<Layer> {
        let palette = self.style.palette;
        let mut layers = vec![
            Layer {
                color: palette.dark(),
                shapes: self.module_shapes(hidden),
            },
            Layer {
                color: palette.finder(),
                shapes: self.finder_shapes(),
            },
        ];
        if let AlignmentStyle::Pattern(pattern) = self.style.alignment {
            let shapes = self
                .visible_alignments(hidden)
                .flat_map(|origin| pattern.shapes_in(self.layout.area(origin, ALIGNMENT_SPAN)))
                .collect();
            layers.push(Layer {
                color: palette.dark(),
                shapes,
            });
        }
        layers
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
                _ => center.distance(b.max()),
            }
        })
    }

    fn module_shapes(&self, hidden: &BTreeSet<Pos>) -> Vec<Shape> {
        let pattern_alignment = matches!(self.style.alignment, AlignmentStyle::Pattern(_));
        let drawn = |x: Option<u8>, y: Option<u8>| {
            x.zip(y).map(|(x, y)| Pos::new(x, y)).is_some_and(|pos| {
                self.symbol.module(pos).is_some_and(|module| {
                    module.dark
                        && module.role != Role::Finder
                        && !(pattern_alignment && module.role == Role::Alignment)
                        && !hidden.contains(&pos)
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

    fn finder_shapes(&self) -> Vec<Shape> {
        self.symbol
            .finders()
            .iter()
            .flat_map(|(_, origin)| {
                self.style
                    .finder
                    .shapes_in(self.layout.area(*origin, FINDER_SPAN))
            })
            .collect()
    }

    fn visible_alignments<'b>(
        &'b self,
        hidden: &'b BTreeSet<Pos>,
    ) -> impl Iterator<Item = Pos> + 'b {
        self.symbol
            .alignment_centers()
            .into_iter()
            .map(|c| Pos::new(c.x - ALIGNMENT_SPAN / 2, c.y - ALIGNMENT_SPAN / 2))
            .filter(move |origin| {
                !hidden.iter().any(|h| {
                    (origin.x..origin.x + ALIGNMENT_SPAN).contains(&h.x)
                        && (origin.y..origin.y + ALIGNMENT_SPAN).contains(&h.y)
                })
            })
    }
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
