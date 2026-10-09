use crate::error::Error;
use crate::geometry::{Disc, Rect, Ring, RoundedRect, Shape};
use crate::units::{Ratio, Roundness};

pub const MIN_CONTRAST: f64 = 3.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const BLACK: Self = Self::rgb(0, 0, 0);
    pub const WHITE: Self = Self::rgb(255, 255, 255);
    pub const TRANSPARENT: Self = Self::rgba(0, 0, 0, 0);

    #[must_use]
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self::rgba(r, g, b, 255)
    }

    #[must_use]
    pub const fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    #[must_use]
    pub fn over_white(self) -> Self {
        let alpha = f64::from(self.a) / 255.0;
        let mix = |c: u8| {
            let v = f64::from(c) * alpha + 255.0 * (1.0 - alpha);
            crate::units::round_clamped(v, 255)
        };
        let channel = |c: u8| u8::try_from(mix(c)).unwrap_or(u8::MAX);
        Self::rgb(channel(self.r), channel(self.g), channel(self.b))
    }

    #[must_use]
    pub fn relative_luminance(self) -> f64 {
        let linear = |c: u8| {
            let s = f64::from(c) / 255.0;
            if s <= 0.040_45 {
                s / 12.92
            } else {
                ((s + 0.055) / 1.055).powf(2.4)
            }
        };
        let c = self.over_white();
        0.2126 * linear(c.r) + 0.7152 * linear(c.g) + 0.0722 * linear(c.b)
    }

    #[must_use]
    pub fn contrast_ratio(self, other: Self) -> f64 {
        let (a, b) = (self.relative_luminance(), other.relative_luminance());
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Palette {
    dark: Color,
    light: Color,
    finder: Color,
}

impl Palette {
    pub fn new(dark: Color, light: Color) -> Result<Self, Error> {
        check_contrast(dark, light)?;
        Ok(Self {
            dark,
            light,
            finder: dark,
        })
    }

    pub fn with_finder(self, finder: Color) -> Result<Self, Error> {
        check_contrast(finder, self.light)?;
        Ok(Self { finder, ..self })
    }

    #[must_use]
    pub const fn dark(self) -> Color {
        self.dark
    }

    #[must_use]
    pub const fn light(self) -> Color {
        self.light
    }

    #[must_use]
    pub const fn finder(self) -> Color {
        self.finder
    }
}

impl Default for Palette {
    fn default() -> Self {
        Self {
            dark: Color::BLACK,
            light: Color::WHITE,
            finder: Color::BLACK,
        }
    }
}

fn check_contrast(dark: Color, light: Color) -> Result<(), Error> {
    let ratio = dark.contrast_ratio(light);
    if dark.relative_luminance() < light.relative_luminance() && ratio >= MIN_CONTRAST {
        Ok(())
    } else {
        Err(Error::InsufficientContrast {
            ratio,
            min: MIN_CONTRAST,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ModuleShape {
    Square,
    Dot { diameter: Ratio },
    Rounded { size: Ratio, roundness: Roundness },
}

impl ModuleShape {
    pub const DOT: Self = Self::Dot {
        diameter: Ratio::or_one(0.85),
    };
    pub const LEGACY_DOT: Self = Self::Dot {
        diameter: Ratio::or_one(0.8),
    };

    #[must_use]
    pub fn shape_in(&self, cell: Rect) -> Option<Shape> {
        let side = cell.width().min(cell.height());
        match *self {
            Self::Square => Some(Shape::Rect(cell)),
            Self::Dot { diameter } => {
                Disc::new(cell.center(), side * diameter.get() / 2.0).map(Shape::Disc)
            }
            Self::Rounded { size, roundness } => {
                let rect = Rect::square(cell.center(), side * size.get());
                Some(Shape::RoundedRect(RoundedRect::new(
                    rect,
                    roundness.radius_for(rect.width()),
                )))
            }
        }
    }

    #[must_use]
    pub const fn min_module_px(&self) -> u32 {
        match self {
            Self::Square => 1,
            Self::Dot { .. } | Self::Rounded { .. } => 3,
        }
    }
}

impl Default for ModuleShape {
    fn default() -> Self {
        Self::DOT
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PatternShape {
    hole: Ratio,
    stone: Ratio,
    outer_corner: Roundness,
    hole_corner: Roundness,
    stone_corner: Roundness,
}

impl PatternShape {
    pub const BRAND_FINDER: Self = Self::ROUNDED_FINDER.with_outer_corner(Roundness::SQUARE);
    pub const ROUNDED_FINDER: Self = Self::uniform(
        Ratio::or_one(228.0 / 308.0),
        Ratio::or_one(138.0 / 308.0),
        Roundness::or_square(120.0 / 308.0),
    );
    pub const SQUARE_FINDER: Self = Self::uniform(
        Ratio::or_one(5.0 / 7.0),
        Ratio::or_one(3.0 / 7.0),
        Roundness::SQUARE,
    );
    pub const ROUNDED_ALIGNMENT: Self = Self::uniform(
        Ratio::or_one(3.0 / 5.0),
        Ratio::or_one(1.0 / 5.0),
        Roundness::or_square(0.5),
    );

    const fn uniform(hole: Ratio, stone: Ratio, corner: Roundness) -> Self {
        Self {
            hole,
            stone,
            outer_corner: corner,
            hole_corner: corner,
            stone_corner: corner,
        }
    }

    pub fn new(hole: Ratio, stone: Ratio, corner: Roundness) -> Result<Self, Error> {
        if stone < hole {
            Ok(Self::uniform(hole, stone, corner))
        } else {
            Err(Error::PatternProportions)
        }
    }

    #[must_use]
    pub const fn with_outer_corner(self, outer_corner: Roundness) -> Self {
        Self {
            outer_corner,
            ..self
        }
    }

    #[must_use]
    pub const fn with_hole_corner(self, hole_corner: Roundness) -> Self {
        Self {
            hole_corner,
            ..self
        }
    }

    #[must_use]
    pub const fn with_stone_corner(self, stone_corner: Roundness) -> Self {
        Self {
            stone_corner,
            ..self
        }
    }

    #[must_use]
    pub fn shapes_in(&self, area: Rect) -> [Shape; 2] {
        let center = area.center();
        let side = area.width().min(area.height());
        let square = |fraction: f64, corner: Roundness| {
            let rect = Rect::square(center, side * fraction);
            RoundedRect::new(rect, corner.radius_for(rect.width()))
        };
        let outer = square(1.0, self.outer_corner);
        let hole = square(self.hole.get(), self.hole_corner);
        let stone = square(self.stone.get(), self.stone_corner);
        let ring = Ring::new(outer, hole).map_or(Shape::RoundedRect(outer), Shape::Ring);
        [ring, Shape::RoundedRect(stone)]
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum AlignmentStyle {
    #[default]
    Modules,
    Pattern(PatternShape),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Style {
    pub module: ModuleShape,
    pub finder: PatternShape,
    pub alignment: AlignmentStyle,
    pub palette: Palette,
}

impl Style {
    pub const BRAND: Self = Self {
        module: ModuleShape::DOT,
        finder: PatternShape::BRAND_FINDER,
        alignment: AlignmentStyle::Pattern(PatternShape::ROUNDED_ALIGNMENT),
        palette: Palette {
            dark: Color::BLACK,
            light: Color::WHITE,
            finder: Color::BLACK,
        },
    };

    pub const LEGACY: Self = Self {
        module: ModuleShape::LEGACY_DOT,
        finder: PatternShape::ROUNDED_FINDER,
        alignment: AlignmentStyle::Modules,
        palette: Self::BRAND.palette,
    };

    pub const CLASSIC: Self = Self {
        module: ModuleShape::Square,
        finder: PatternShape::SQUARE_FINDER,
        alignment: AlignmentStyle::Modules,
        palette: Self::BRAND.palette,
    };
}

impl Default for Style {
    fn default() -> Self {
        Self::BRAND
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::Point;

    #[test]
    fn contrast_follows_wcag() {
        assert!((Color::BLACK.contrast_ratio(Color::WHITE) - 21.0).abs() < 1e-9);
        assert!((Color::WHITE.contrast_ratio(Color::WHITE) - 1.0).abs() < 1e-9);
        assert_eq!(Color::TRANSPARENT.over_white(), Color::WHITE);
        let dark = Color::rgb(10, 10, 10).relative_luminance();
        assert!((dark - 10.0 / 255.0 / 12.92).abs() < 1e-12);
        let mid = Color::rgb(128, 128, 128).relative_luminance();
        assert!((mid - ((128.0 / 255.0 + 0.055) / 1.055_f64).powf(2.4)).abs() < 1e-12);
        assert_eq!(
            Color::rgba(0, 0, 0, 128).over_white(),
            Color::rgb(127, 127, 127)
        );
    }

    #[test]
    fn palette_rejects_low_contrast_and_inverted_colors() {
        assert!(Palette::new(Color::BLACK, Color::WHITE).is_ok());
        assert!(Palette::new(Color::rgb(30, 60, 140), Color::WHITE).is_ok());
        assert!(matches!(
            Palette::new(Color::rgb(200, 200, 200), Color::WHITE),
            Err(Error::InsufficientContrast { .. })
        ));
        assert!(Palette::new(Color::WHITE, Color::BLACK).is_err());
        assert!(Palette::new(Color::BLACK, Color::TRANSPARENT).is_ok());
        let palette = Palette::default();
        assert!(palette.with_finder(Color::rgb(250, 250, 250)).is_err());
        let blue = Color::rgb(0, 40, 160);
        assert_eq!(
            palette.with_finder(blue).map(Palette::finder).ok(),
            Some(blue)
        );
    }

    #[test]
    fn brand_constants_are_valid_ratios() {
        let check = |p: PatternShape| {
            assert!(p.stone < p.hole);
            assert!(p.hole.get() < 1.0);
            assert!(p.outer_corner.get() <= 1.0 && p.stone_corner.get() >= 0.0);
        };
        check(PatternShape::BRAND_FINDER);
        check(PatternShape::ROUNDED_FINDER);
        check(PatternShape::SQUARE_FINDER);
        check(PatternShape::ROUNDED_ALIGNMENT);
        assert!((PatternShape::BRAND_FINDER.hole.get() - 228.0 / 308.0).abs() < 1e-12);
        assert_eq!(PatternShape::BRAND_FINDER.outer_corner, Roundness::SQUARE);
        assert_eq!(
            PatternShape::BRAND_FINDER.with_outer_corner(PatternShape::ROUNDED_FINDER.outer_corner),
            PatternShape::ROUNDED_FINDER
        );
        assert_eq!(
            ModuleShape::DOT,
            ModuleShape::Dot {
                diameter: Ratio::or_one(0.85)
            }
        );
        assert_eq!(
            ModuleShape::LEGACY_DOT,
            ModuleShape::Dot {
                diameter: Ratio::or_one(0.8)
            }
        );
        assert_eq!(Style::default(), Style::BRAND);
        assert_eq!(Style::BRAND.palette, Palette::default());
    }

    #[test]
    fn pattern_proportions_are_validated() {
        let r = |v| Ratio::new(v).expect("ratio");
        let corner = Roundness::or_square(0.1);
        assert!(PatternShape::new(r(0.5), r(0.6), corner).is_err());
        assert!(PatternShape::new(r(0.5), r(0.5), corner).is_err());
        assert!(PatternShape::new(r(0.7), r(0.4), corner).is_ok());
    }

    #[test]
    fn pattern_shapes_nest_and_are_centered() {
        let area = Rect::from_origin(Point::new(10.0, 20.0), 70.0, 70.0);
        let [ring, stone] = PatternShape::ROUNDED_FINDER.shapes_in(area);
        let ring = as_ring(ring).expect("ring");
        assert_eq!(ring.outer().rect(), area);
        assert_eq!(ring.inner().rect().center(), area.center());
        assert_eq!(stone.bounds().center(), area.center());
        assert!((ring.inner().rect().width() - 70.0 * 228.0 / 308.0).abs() < 1e-9);
        assert!((ring.outer().radius() - 70.0 * 60.0 / 308.0).abs() < 1e-9);
        assert!((stone.bounds().width() - 70.0 * 138.0 / 308.0).abs() < 1e-9);
        let sharp = PatternShape::BRAND_FINDER;
        let ring = as_ring(sharp.shapes_in(area)[0]).expect("ring");
        assert!((ring.outer().radius() - 0.0).abs() < 1e-12);
        assert!((ring.inner().radius() - 70.0 * 228.0 / 308.0 * 60.0 / 308.0).abs() < 1e-9);
        let custom = sharp
            .with_hole_corner(Roundness::CIRCLE)
            .with_stone_corner(Roundness::SQUARE);
        let [ring, stone] = custom.shapes_in(area);
        let ring = as_ring(ring).expect("ring");
        assert!((ring.inner().radius() - ring.inner().rect().width() / 2.0).abs() < 1e-9);
        assert!((stone.area() - stone.bounds().area()).abs() < 1e-12);
    }

    fn as_ring(shape: Shape) -> Option<Ring> {
        match shape {
            Shape::Ring(ring) => Some(ring),
            _ => None,
        }
    }

    fn as_disc(shape: Shape) -> Option<Disc> {
        match shape {
            Shape::Disc(disc) => Some(disc),
            _ => None,
        }
    }

    #[test]
    fn module_shapes_stay_inside_their_cell() {
        let cell = Rect::from_origin(Point::new(4.0, 8.0), 11.0, 11.0);
        let shapes = [
            ModuleShape::Square,
            ModuleShape::DOT,
            ModuleShape::Rounded {
                size: Ratio::or_one(0.9),
                roundness: Roundness::or_square(0.5),
            },
        ];
        for module in shapes {
            let shape = module.shape_in(cell).expect("shape");
            let b = shape.bounds();
            assert!(
                b.min().x >= cell.min().x && b.max().x <= cell.max().x,
                "{module:?}"
            );
            assert!(
                b.min().y >= cell.min().y && b.max().y <= cell.max().y,
                "{module:?}"
            );
            assert_eq!(b.center(), cell.center());
        }
        let dot = ModuleShape::DOT
            .shape_in(cell)
            .and_then(as_disc)
            .expect("disc");
        assert!((dot.radius() - 11.0 * 0.85 / 2.0).abs() < 1e-12);
    }
}
