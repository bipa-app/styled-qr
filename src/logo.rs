use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use image::imageops::FilterType;
use image::{GenericImageView, ImageBuffer, Rgba, RgbaImage};

use crate::error::Error;
use crate::geometry::{Point, Rect, RoundedRect, Shape};
use crate::style::Color;
use crate::units::{Padding, Ratio, Roundness, floor_clamped, round_clamped};

pub(crate) type PremultipliedImage = ImageBuffer<Rgba<f64>, Vec<f64>>;

const OPAQUE_ALPHA: f64 = 0.1;
const CACHED_RENDITIONS: usize = 16;

type RenditionKey = (u32, u32, Clearance, Option<Plate>);
type RenditionCache = Arc<Mutex<VecDeque<(RenditionKey, Arc<Rendition>)>>>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LogoSize {
    Width(Ratio),
    Area(Ratio),
}

impl LogoSize {
    pub const LEGACY: Self = Self::Area(Ratio::or_one(0.05));

    fn side_px(self, symbol_px: f64, finder_px: f64) -> f64 {
        match self {
            Self::Width(ratio) => symbol_px * ratio.get(),
            Self::Area(ratio) => {
                let free = symbol_px * symbol_px - 3.0 * finder_px * finder_px;
                (free.max(0.0) * ratio.get()).sqrt()
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Clearance {
    #[default]
    Hull,
    Footprint,
    Bounds,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum OverBudget {
    Reject,
    #[default]
    Shrink,
}

impl OverBudget {
    pub(crate) const fn fallback_scales(self) -> &'static [f64] {
        match self {
            Self::Reject => &[],
            Self::Shrink => &[0.9, 0.8, 0.7, 0.6, 0.5],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Plate {
    pub color: Color,
    pub margin: Ratio,
    pub roundness: Roundness,
}

#[derive(Clone, Debug)]
pub struct Logo {
    image: Arc<RgbaImage>,
    size: LogoSize,
    padding: Padding,
    clearance: Clearance,
    error_budget: Ratio,
    over_budget: OverBudget,
    plate: Option<Plate>,
    renditions: RenditionCache,
}

impl Logo {
    pub const DEFAULT_ERROR_BUDGET: Ratio = Ratio::or_one(0.75);

    pub fn new<I: GenericImageView<Pixel = Rgba<u8>>>(image: &I) -> Result<Self, Error> {
        let (w, h) = image.dimensions();
        if w == 0 || h == 0 {
            return Err(Error::EmptyLogo);
        }
        let image = Arc::new(RgbaImage::from_fn(w, h, |x, y| image.get_pixel(x, y)));
        Ok(Self {
            image,
            size: LogoSize::LEGACY,
            padding: Padding::default(),
            clearance: Clearance::default(),
            error_budget: Self::DEFAULT_ERROR_BUDGET,
            over_budget: OverBudget::default(),
            plate: None,
            renditions: RenditionCache::default(),
        })
    }

    #[must_use]
    pub fn with_size(self, size: LogoSize) -> Self {
        Self { size, ..self }
    }

    #[must_use]
    pub fn with_padding(self, padding: Padding) -> Self {
        Self { padding, ..self }
    }

    #[must_use]
    pub fn with_clearance(self, clearance: Clearance) -> Self {
        Self { clearance, ..self }
    }

    #[must_use]
    pub fn with_error_budget(self, error_budget: Ratio) -> Self {
        Self {
            error_budget,
            ..self
        }
    }

    #[must_use]
    pub fn with_over_budget(self, over_budget: OverBudget) -> Self {
        Self {
            over_budget,
            ..self
        }
    }

    #[must_use]
    pub fn with_plate(self, plate: Plate) -> Self {
        Self {
            plate: Some(plate),
            ..self
        }
    }

    #[must_use]
    pub const fn plate(&self) -> Option<Plate> {
        self.plate
    }

    #[must_use]
    pub const fn over_budget(&self) -> OverBudget {
        self.over_budget
    }

    #[must_use]
    pub const fn size(&self) -> LogoSize {
        self.size
    }

    #[must_use]
    pub const fn padding(&self) -> Padding {
        self.padding
    }

    #[must_use]
    pub const fn clearance(&self) -> Clearance {
        self.clearance
    }

    #[must_use]
    pub const fn error_budget(&self) -> Ratio {
        self.error_budget
    }

    pub(crate) fn place(
        &self,
        canvas_px: u32,
        symbol_px: f64,
        finder_px: f64,
        scale: f64,
    ) -> Option<PlacedLogo> {
        let side = fit_parity(
            round_clamped(self.size.side_px(symbol_px, finder_px) * scale, canvas_px),
            canvas_px,
        );
        let (iw, ih) = self.image.dimensions();
        let scale = f64::from(side) / f64::from(iw.max(ih));
        let tw = fit_parity(round_clamped(f64::from(iw) * scale, side), canvas_px);
        let th = fit_parity(round_clamped(f64::from(ih) * scale, side), canvas_px);
        if tw < 2 || th < 2 {
            return None;
        }
        let rendition = self.rendition(tw, th);
        let (width, height) = rendition.image.dimensions();
        (width <= canvas_px && height <= canvas_px).then(|| PlacedLogo {
            origin: ((canvas_px - width) / 2, (canvas_px - height) / 2),
            rendition,
        })
    }

    fn rendition(&self, width: u32, height: u32) -> Arc<Rendition> {
        let key = (width, height, self.clearance, self.plate);
        if let Ok(cache) = self.renditions.lock()
            && let Some((_, cached)) = cache.iter().find(|(k, _)| *k == key)
        {
            return Arc::clone(cached);
        }
        let resized = resize_premultiplied(&self.image, width, height);
        let image = match self.plate {
            Some(plate) => on_plate(&resized, plate),
            None => resized,
        };
        let footprint = match self.clearance {
            Clearance::Bounds => Footprint::Bounds,
            Clearance::Footprint => Footprint::Pixels(edge_pixels(&image)),
            Clearance::Hull => Footprint::Hull(convex_hull(&edge_pixels(&image))),
        };
        let rendition = Arc::new(Rendition { image, footprint });
        if let Ok(mut cache) = self.renditions.lock() {
            if cache.len() >= CACHED_RENDITIONS {
                cache.pop_front();
            }
            cache.push_back((key, Arc::clone(&rendition)));
        }
        rendition
    }
}

fn on_plate(logo: &PremultipliedImage, plate: Plate) -> PremultipliedImage {
    let (width, height) = logo.dimensions();
    let margin = round_clamped(
        f64::from(width.max(height)) * plate.margin.get(),
        u32::MAX / 4,
    );
    let (plate_w, plate_h) = (width + 2 * margin, height + 2 * margin);
    let rect = Rect::from_origin(Point::new(0.0, 0.0), f64::from(plate_w), f64::from(plate_h));
    let shape = Shape::RoundedRect(RoundedRect::new(
        rect,
        plate.roundness.radius_for(rect.width().min(rect.height())),
    ));
    let [r, g, b, a] =
        [plate.color.r, plate.color.g, plate.color.b, plate.color.a].map(|c| f64::from(c) / 255.0);
    let mut image = PremultipliedImage::from_fn(plate_w, plate_h, |x, y| {
        let coverage = shape.pixel_coverage(x, y) * a;
        Rgba([r * coverage, g * coverage, b * coverage, coverage])
    });
    for (x, y, &Rgba([sr, sg, sb, sa])) in logo.enumerate_pixels() {
        if let Some(dst) = image.get_pixel_mut_checked(x + margin, y + margin) {
            let Rgba([dr, dg, db, da]) = *dst;
            let keep = 1.0 - sa;
            *dst = Rgba([
                sr + dr * keep,
                sg + dg * keep,
                sb + db * keep,
                sa + da * keep,
            ]);
        }
    }
    image
}

fn fit_parity(px: u32, canvas_px: u32) -> u32 {
    if (canvas_px - px.min(canvas_px)).is_multiple_of(2) {
        px.min(canvas_px)
    } else {
        px.saturating_sub(1)
    }
}

#[derive(Clone, Debug, PartialEq)]
enum Footprint {
    Bounds,
    Pixels(Vec<(u32, u32)>),
    Hull(Vec<Point>),
}

#[derive(Clone, Debug, PartialEq)]
struct Rendition {
    image: PremultipliedImage,
    footprint: Footprint,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct PlacedLogo {
    origin: (u32, u32),
    rendition: Arc<Rendition>,
}

impl PlacedLogo {
    pub(crate) const fn origin(&self) -> (u32, u32) {
        self.origin
    }

    pub(crate) fn image(&self) -> &PremultipliedImage {
        &self.rendition.image
    }

    pub(crate) fn rect(&self) -> Rect {
        let (w, h) = self.image().dimensions();
        Rect::from_origin(
            Point::new(f64::from(self.origin.0), f64::from(self.origin.1)),
            f64::from(w),
            f64::from(h),
        )
    }

    pub(crate) fn distance(&self, p: Point) -> f64 {
        let local = Point::new(
            p.x - f64::from(self.origin.0),
            p.y - f64::from(self.origin.1),
        );
        match &self.rendition.footprint {
            Footprint::Bounds => self.rect().distance_to(p),
            Footprint::Pixels(edge) => {
                if self.is_opaque_at(p) {
                    return 0.0;
                }
                edge.iter()
                    .map(|&(x, y)| pixel_rect(x, y).distance_to(local))
                    .fold(f64::INFINITY, f64::min)
            }
            Footprint::Hull(hull) => polygon_distance(hull, local),
        }
    }

    pub(crate) fn reads_dark(&self, cell: Rect, background: Color) -> bool {
        let center = cell.center();
        let step = cell.width() / 4.0;
        let offsets = [-step, 0.0, step];
        let [br, bg, bb] = [background.r, background.g, background.b].map(|c| f64::from(c) / 255.0);
        let background_alpha = f64::from(background.a) / 255.0;
        let luma: f64 = offsets
            .iter()
            .flat_map(|dy| {
                offsets
                    .iter()
                    .map(move |dx| Point::new(center.x + dx, center.y + dy))
            })
            .map(|point| {
                let Rgba([red, green, blue, alpha]) = self.pixel_at(point);
                let under = background_alpha * (1.0 - alpha);
                let page = 1.0 - alpha - under;
                0.2126 * (red + br * under + page)
                    + 0.7152 * (green + bg * under + page)
                    + 0.0722 * (blue + bb * under + page)
            })
            .sum::<f64>()
            / 9.0;
        luma < 0.5
    }

    fn pixel_at(&self, point: Point) -> Rgba<f64> {
        self.pixel_index(point)
            .and_then(|(x, y)| self.image().get_pixel_checked(x, y).copied())
            .unwrap_or(Rgba([0.0; 4]))
    }

    fn is_opaque_at(&self, point: Point) -> bool {
        self.pixel_index(point)
            .is_some_and(|(x, y)| alpha(self.image(), x, y) >= OPAQUE_ALPHA)
    }

    fn pixel_index(&self, point: Point) -> Option<(u32, u32)> {
        let rect = self.rect();
        let (width, height) = self.image().dimensions();
        (rect.distance_to(point) <= 0.0).then(|| {
            (
                floor_clamped(point.x - rect.min().x, width.saturating_sub(1)),
                floor_clamped(point.y - rect.min().y, height.saturating_sub(1)),
            )
        })
    }
}

fn pixel_rect(x: u32, y: u32) -> Rect {
    Rect::from_origin(Point::new(f64::from(x), f64::from(y)), 1.0, 1.0)
}

fn convex_hull(edge: &[(u32, u32)]) -> Vec<Point> {
    let mut points: Vec<Point> = edge
        .iter()
        .flat_map(|&(x, y)| {
            let (left, top) = (f64::from(x), f64::from(y));
            [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)]
                .map(|(dx, dy)| Point::new(left + dx, top + dy))
        })
        .collect();
    points.sort_by(|a, b| a.x.total_cmp(&b.x).then(a.y.total_cmp(&b.y)));
    points.dedup();
    let half_hull = |points: &mut dyn Iterator<Item = Point>| {
        let mut chain: Vec<Point> = Vec::new();
        for p in points {
            while let [.., a, b] = *chain.as_slice() {
                if cross(a, b, p) > 0.0 {
                    break;
                }
                chain.pop();
            }
            chain.push(p);
        }
        chain.pop();
        chain
    };
    let mut hull = half_hull(&mut points.iter().copied());
    hull.extend(half_hull(&mut points.iter().rev().copied()));
    hull
}

fn cross(o: Point, a: Point, b: Point) -> f64 {
    (a.x - o.x) * (b.y - o.y) - (a.y - o.y) * (b.x - o.x)
}

fn polygon_distance(hull: &[Point], p: Point) -> f64 {
    let edges = || hull.iter().zip(hull.iter().cycle().skip(1));
    if hull.len() >= 3 && edges().all(|(a, b)| cross(*a, *b, p) >= 0.0) {
        return 0.0;
    }
    edges()
        .map(|(a, b)| segment_distance(*a, *b, p))
        .fold(f64::INFINITY, f64::min)
}

fn segment_distance(a: Point, b: Point, p: Point) -> f64 {
    let (dx, dy) = (b.x - a.x, b.y - a.y);
    let length = dx * dx + dy * dy;
    let t = if length > 0.0 {
        (((p.x - a.x) * dx + (p.y - a.y) * dy) / length).clamp(0.0, 1.0)
    } else {
        0.0
    };
    p.distance(Point::new(a.x + t * dx, a.y + t * dy))
}

fn alpha(image: &PremultipliedImage, x: u32, y: u32) -> f64 {
    image
        .get_pixel_checked(x, y)
        .map_or(0.0, |&Rgba([_, _, _, a])| a)
}

fn edge_pixels(image: &PremultipliedImage) -> Vec<(u32, u32)> {
    let opaque = |x: Option<u32>, y: Option<u32>| match (x, y) {
        (Some(x), Some(y)) => alpha(image, x, y) >= OPAQUE_ALPHA,
        _ => false,
    };
    image
        .enumerate_pixels()
        .filter(|(x, y, Rgba([_, _, _, a]))| {
            *a >= OPAQUE_ALPHA
                && !(opaque(x.checked_sub(1), Some(*y))
                    && opaque(x.checked_add(1), Some(*y))
                    && opaque(Some(*x), y.checked_sub(1))
                    && opaque(Some(*x), y.checked_add(1)))
        })
        .map(|(x, y, _)| (x, y))
        .collect()
}

fn resize_premultiplied(image: &RgbaImage, width: u32, height: u32) -> PremultipliedImage {
    let factor = (image.width() / width.max(1))
        .min(image.height() / height.max(1))
        .max(1);
    let prefiltered = box_downsample(image, factor);
    let mut resized = image::imageops::resize(&prefiltered, width, height, FilterType::Lanczos3);
    for pixel in resized.pixels_mut() {
        let Rgba([r, g, b, a]) = *pixel;
        let a = a.clamp(0.0, 1.0);
        *pixel = Rgba([r.clamp(0.0, a), g.clamp(0.0, a), b.clamp(0.0, a), a]);
    }
    resized
}

fn box_downsample(image: &RgbaImage, factor: u32) -> PremultipliedImage {
    let (width, height) = (image.width() / factor, image.height() / factor);
    PremultipliedImage::from_fn(width.max(1), height.max(1), |col, row| {
        let mut sum = [0.0; 4];
        let mut samples = 0u32;
        for dy in 0..factor {
            for dx in 0..factor {
                let pixel = image.get_pixel_checked(col * factor + dx, row * factor + dy);
                if let Some(Rgba(channels)) = pixel {
                    let [red, green, blue, alpha] = channels.map(|c| f64::from(c) / 255.0);
                    let premultiplied = [red * alpha, green * alpha, blue * alpha, alpha];
                    for (total, value) in sum.iter_mut().zip(premultiplied) {
                        *total += value;
                    }
                    samples += 1;
                }
            }
        }
        let samples = f64::from(samples.max(1));
        Rgba(sum.map(|total| total / samples))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::units::Padding;

    fn disc_logo(size: u32) -> RgbaImage {
        let c = f64::from(size) / 2.0;
        RgbaImage::from_fn(size, size, |x, y| {
            let d = (f64::from(x) + 0.5 - c).hypot(f64::from(y) + 0.5 - c);
            if d <= c * 0.9 {
                Rgba([0, 0, 0, 255])
            } else {
                Rgba([255, 0, 0, 0])
            }
        })
    }

    #[test]
    fn renditions_are_reused_per_size_and_clearance() {
        let logo = Logo::new(&disc_logo(300)).expect("logo");
        let first = logo.place(1000, 800.0, 70.0, 1.0).expect("placed");
        let again = logo.place(1000, 800.0, 70.0, 1.0).expect("placed");
        assert!(Arc::ptr_eq(&first.rendition, &again.rendition));
        let cloned = logo.clone().with_padding(Padding::new(2).expect("padding"));
        let shared = cloned.place(1000, 800.0, 70.0, 1.0).expect("placed");
        assert!(Arc::ptr_eq(&first.rendition, &shared.rendition));
        let bounds = logo.clone().with_clearance(Clearance::Bounds);
        let other = bounds.place(1000, 800.0, 70.0, 1.0).expect("placed");
        assert!(!Arc::ptr_eq(&first.rendition, &other.rendition));
        assert_eq!(first.image(), other.image());
        let smaller = logo.place(1000, 800.0, 70.0, 0.5).expect("placed");
        assert!(!Arc::ptr_eq(&first.rendition, &smaller.rendition));
        let fresh = Logo::new(&disc_logo(300))
            .expect("logo")
            .place(1000, 800.0, 70.0, 1.0)
            .expect("placed");
        assert_eq!(fresh, first);
    }

    #[test]
    fn rendition_cache_is_bounded() {
        let logo = Logo::new(&disc_logo(64)).expect("logo");
        for step in 0..40u32 {
            let scale = 0.3 + f64::from(step) * 0.015;
            assert!(logo.place(1000, 800.0, 70.0, scale).is_some());
        }
        let cached = logo.renditions.lock().map_or(0, |c| c.len());
        assert_eq!(cached, CACHED_RENDITIONS);
    }

    #[test]
    fn plate_surrounds_the_logo() {
        let image = RgbaImage::from_pixel(20, 20, Rgba([200, 0, 0, 255]));
        let plate = Plate {
            color: Color::rgb(10, 20, 30),
            margin: Ratio::or_one(0.25),
            roundness: Roundness::SQUARE,
        };
        let logo = Logo::new(&image)
            .expect("logo")
            .with_size(LogoSize::Width(Ratio::or_one(0.2)))
            .with_plate(plate);
        assert_eq!(logo.plate(), Some(plate));
        let placed = logo.place(100, 100.0, 7.0, 1.0).expect("placed");
        assert_eq!(placed.image().dimensions(), (30, 30));
        assert_eq!(placed.origin(), (35, 35));
        let pixel = |x: u32, y: u32| placed.image().get_pixel(x, y).0;
        let close = |a: [f64; 4], b: [f64; 4]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9);
        assert!(close(
            pixel(0, 0),
            [10.0 / 255.0, 20.0 / 255.0, 30.0 / 255.0, 1.0]
        ));
        assert!(close(pixel(15, 15), [200.0 / 255.0, 0.0, 0.0, 1.0]));
        assert!(close(
            pixel(4, 15),
            [10.0 / 255.0, 20.0 / 255.0, 30.0 / 255.0, 1.0]
        ));
        assert!(close(pixel(5, 15), [200.0 / 255.0, 0.0, 0.0, 1.0]));
        assert!((placed.distance(Point::new(30.0, 50.0)) - 5.0).abs() < 1e-9);
        let plain = Logo::new(&image)
            .expect("logo")
            .with_size(LogoSize::Width(Ratio::or_one(0.2)));
        let bare = plain.place(100, 100.0, 7.0, 1.0).expect("placed");
        assert_eq!(bare.image().dimensions(), (20, 20));
        assert!(!Arc::ptr_eq(&bare.rendition, &placed.rendition));
    }

    #[test]
    fn rounded_plate_has_transparent_corners() {
        let image = RgbaImage::from_pixel(40, 40, Rgba([0, 0, 0, 0]));
        let logo = Logo::new(&image)
            .expect("logo")
            .with_size(LogoSize::Width(Ratio::or_one(0.4)))
            .with_plate(Plate {
                color: Color::WHITE,
                margin: Ratio::or_one(0.1),
                roundness: Roundness::CIRCLE,
            });
        let placed = logo.place(100, 100.0, 7.0, 1.0).expect("placed");
        assert_eq!(placed.image().dimensions(), (48, 48));
        let alpha = |x: u32, y: u32| placed.image().get_pixel(x, y).0[3];
        assert!(alpha(0, 0) < 1e-9);
        assert!((alpha(24, 24) - 1.0).abs() < 1e-9);
        assert!(alpha(0, 24) > 0.0 && alpha(0, 24) <= 1.0);
        let total: f64 = placed.image().pixels().map(|p| p.0[3]).sum();
        assert!((total - std::f64::consts::PI * 24.0 * 24.0).abs() < 1e-6);
    }

    #[test]
    fn plate_larger_than_the_canvas_is_skipped() {
        let image = RgbaImage::from_pixel(20, 20, Rgba([0, 0, 0, 255]));
        let logo = Logo::new(&image)
            .expect("logo")
            .with_size(LogoSize::Width(Ratio::ONE))
            .with_plate(Plate {
                color: Color::WHITE,
                margin: Ratio::ONE,
                roundness: Roundness::SQUARE,
            });
        assert!(logo.place(100, 100.0, 7.0, 1.0).is_none());
    }

    #[test]
    fn rejects_empty_images() {
        assert!(matches!(
            Logo::new(&RgbaImage::new(0, 10)),
            Err(Error::EmptyLogo)
        ));
        assert!(Logo::new(&RgbaImage::new(1, 1)).is_ok());
    }

    #[test]
    fn placement_is_exactly_centered_for_both_canvas_parities() {
        let logo = Logo::new(&disc_logo(300)).expect("logo");
        for canvas in [990u32, 991, 1166, 1167] {
            let placed = logo
                .place(canvas, f64::from(canvas) * 0.8, 70.0, 1.0)
                .expect("placed");
            let rect = placed.rect();
            assert_eq!(
                rect.center(),
                Point::new(f64::from(canvas) / 2.0, f64::from(canvas) / 2.0)
            );
            assert!((rect.width() - rect.height()).abs() < 1e-12);
        }
    }

    #[test]
    fn preserves_aspect_ratio() {
        let wide = RgbaImage::from_pixel(400, 200, Rgba([0, 0, 0, 255]));
        let logo = Logo::new(&wide)
            .expect("logo")
            .with_size(LogoSize::Width(Ratio::or_one(0.25)));
        let placed = logo.place(1000, 800.0, 70.0, 1.0).expect("placed");
        let (w, h) = placed.image().dimensions();
        assert_eq!(w, 200);
        assert_eq!(h, 100);
        assert_eq!(placed.origin(), (400, 450));
        let half = logo.place(1000, 800.0, 70.0, 0.5).expect("placed");
        assert_eq!(half.image().dimensions(), (100, 50));
    }

    #[test]
    fn legacy_size_matches_the_area_formula() {
        let symbol_px = 37.0 * 22.0;
        let finder_px = 7.0 * 22.0;
        let side = LogoSize::LEGACY.side_px(symbol_px, finder_px);
        let expected = ((symbol_px * symbol_px - 3.0 * finder_px * finder_px) * 0.05).sqrt();
        assert!((side - expected).abs() < 1e-9);
    }

    #[test]
    fn builder_methods_set_their_field() {
        let logo = Logo::new(&disc_logo(10)).expect("logo");
        let budget = Ratio::or_one(0.3);
        assert_eq!(
            logo.clone().with_error_budget(budget).error_budget(),
            budget
        );
        assert_eq!(logo.error_budget(), Logo::DEFAULT_ERROR_BUDGET);
        let padding = Padding::new(3).expect("padding");
        assert_eq!(logo.clone().with_padding(padding).padding(), padding);
        assert_eq!(
            logo.clone().with_clearance(Clearance::Bounds).clearance(),
            Clearance::Bounds
        );
        assert_eq!(
            logo.clone()
                .with_over_budget(OverBudget::Reject)
                .over_budget(),
            OverBudget::Reject
        );
        let size = LogoSize::Width(Ratio::or_one(0.2));
        assert_eq!(logo.with_size(size).size(), size);
    }

    #[test]
    fn flat_logos_that_round_to_one_pixel_are_skipped() {
        let wide = RgbaImage::from_pixel(200, 1, Rgba([0, 0, 0, 255]));
        let logo = Logo::new(&wide)
            .expect("logo")
            .with_size(LogoSize::Width(Ratio::or_one(0.2)));
        assert!(logo.place(1000, 1000.0, 70.0, 1.0).is_none());
        let tall = RgbaImage::from_pixel(1, 200, Rgba([0, 0, 0, 255]));
        let logo = Logo::new(&tall)
            .expect("logo")
            .with_size(LogoSize::Width(Ratio::or_one(0.2)));
        assert!(logo.place(1000, 1000.0, 70.0, 1.0).is_none());
        let square = RgbaImage::from_pixel(4, 4, Rgba([0, 0, 0, 255]));
        let logo = Logo::new(&square)
            .expect("logo")
            .with_size(LogoSize::Width(Ratio::or_one(0.002)));
        assert!(logo.place(1000, 1000.0, 70.0, 1.0).is_some());
    }

    #[test]
    fn footprint_distance_is_exact_to_opaque_pixels() {
        let mut image = RgbaImage::new(10, 10);
        for (x, y) in [
            (3, 4),
            (4, 4),
            (5, 4),
            (3, 5),
            (4, 5),
            (5, 5),
            (3, 6),
            (4, 6),
            (5, 6),
        ] {
            image.put_pixel(x, y, Rgba([0, 0, 0, 255]));
        }
        let logo = Logo::new(&image)
            .expect("logo")
            .with_size(LogoSize::Width(Ratio::or_one(0.1)));
        let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
        let hull = logo.place(100, 100.0, 7.0, 1.0).expect("placed");
        assert!(close(hull.distance(Point::new(49.5, 50.5)), 0.0));
        assert!(close(hull.distance(Point::new(46.0, 50.5)), 2.0));
        let placed = logo
            .clone()
            .with_clearance(Clearance::Footprint)
            .place(100, 100.0, 7.0, 1.0)
            .expect("placed");
        assert_eq!(placed.origin(), (45, 45));
        assert!(close(placed.distance(Point::new(49.5, 50.5)), 0.0));
        assert!(close(placed.distance(Point::new(48.2, 49.9)), 0.0));
        assert!(close(placed.distance(Point::new(46.0, 50.5)), 2.0));
        assert!(close(placed.distance(Point::new(60.0, 50.5)), 9.0));
        assert!(close(
            placed.distance(Point::new(45.0, 46.0)),
            3.0_f64.hypot(3.0)
        ));
        assert!(close(placed.distance(Point::new(49.5, 30.0)), 19.0));
        let bounds = logo.with_clearance(Clearance::Bounds);
        let placed = bounds.place(100, 100.0, 7.0, 1.0).expect("placed");
        assert!(close(placed.distance(Point::new(46.0, 50.5)), 0.0));
        assert!(close(placed.distance(Point::new(30.0, 50.0)), 15.0));
    }

    #[test]
    fn hull_clearance_fills_concavities() {
        let mut image = RgbaImage::new(30, 30);
        for y in 0..30 {
            for x in 0..30 {
                if x < 6 || y < 6 {
                    image.put_pixel(x, y, Rgba([0, 0, 0, 255]));
                }
            }
        }
        let logo = Logo::new(&image)
            .expect("logo")
            .with_size(LogoSize::Width(Ratio::or_one(0.3)));
        let at = |clearance| {
            logo.clone()
                .with_clearance(clearance)
                .place(100, 100.0, 7.0, 1.0)
                .expect("placed")
        };
        let (hull, pixels, bounds) = (
            at(Clearance::Hull),
            at(Clearance::Footprint),
            at(Clearance::Bounds),
        );
        assert_eq!(hull.origin(), (35, 35));
        let close = |a: f64, b: f64| (a - b).abs() < 1e-9;
        let notch = Point::new(50.0, 50.0);
        assert!(close(hull.distance(notch), 0.0));
        assert!(close(bounds.distance(notch), 0.0));
        assert!(close(pixels.distance(notch), 9.0));
        let corner = Point::new(64.0, 64.0);
        let diagonal = (64.0 + 64.0 - 106.0) / std::f64::consts::SQRT_2;
        assert!(hull.distance(corner) > 0.0);
        assert!(close(bounds.distance(corner), 0.0));
        assert!(close(hull.distance(corner), diagonal));
        let left = Point::new(30.0, 50.0);
        for placed in [&hull, &pixels, &bounds] {
            assert!(close(placed.distance(left), 5.0));
        }
        assert!(close(hull.distance(Point::new(36.0, 36.0)), 0.0));
    }

    #[test]
    fn hull_of_nothing_is_infinitely_far() {
        assert!(polygon_distance(&[], Point::new(0.0, 0.0)).is_infinite());
        let segment = [Point::new(0.0, 0.0), Point::new(10.0, 0.0)];
        assert!((polygon_distance(&segment, Point::new(5.0, 3.0)) - 3.0).abs() < 1e-12);
        assert!(
            (segment_distance(segment[0], segment[0], Point::new(3.0, 4.0)) - 5.0).abs() < 1e-12
        );
        let hull = convex_hull(&[(0, 0), (2, 0), (0, 2), (2, 2), (1, 1)]);
        assert_eq!(hull.len(), 4);
        assert!((polygon_distance(&hull, Point::new(1.5, 1.5))).abs() < 1e-12);
        assert!((polygon_distance(&hull, Point::new(5.0, 1.5)) - 2.0).abs() < 1e-12);
    }

    #[test]
    fn reads_dark_matches_composited_luminance() {
        let luma = |logo: [u8; 4], bg: Color| {
            let logo_alpha = f64::from(logo[3]) / 255.0;
            let bg_alpha = f64::from(bg.a) / 255.0;
            let channel = |l: u8, b: u8| {
                let under = f64::from(b) / 255.0 * bg_alpha + (1.0 - bg_alpha);
                f64::from(l) / 255.0 * logo_alpha + under * (1.0 - logo_alpha)
            };
            0.2126 * channel(logo[0], bg.r)
                + 0.7152 * channel(logo[1], bg.g)
                + 0.0722 * channel(logo[2], bg.b)
        };
        let logos = [
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 255],
            [100, 100, 100, 255],
            [150, 150, 150, 255],
            [0, 0, 0, 100],
            [0, 0, 0, 160],
            [255, 255, 255, 90],
            [255, 255, 255, 170],
            [40, 200, 90, 128],
        ];
        let backgrounds = [
            Color::WHITE,
            Color::BLACK,
            Color::rgb(128, 128, 128),
            Color::rgba(0, 0, 0, 128),
            Color::rgba(200, 30, 30, 200),
            Color::TRANSPARENT,
        ];
        let cell = Rect::square(Point::new(50.0, 50.0), 8.0);
        let mut checked = 0;
        for logo in logos {
            let image = RgbaImage::from_pixel(10, 10, Rgba(logo));
            let placed = Logo::new(&image)
                .expect("logo")
                .with_size(LogoSize::Width(Ratio::or_one(0.1)))
                .place(100, 100.0, 7.0, 1.0)
                .expect("placed");
            for bg in backgrounds {
                let expected = luma(logo, bg);
                if (expected - 0.5).abs() > 0.01 {
                    assert_eq!(
                        placed.reads_dark(cell, bg),
                        expected < 0.5,
                        "{logo:?} on {bg:?}"
                    );
                    checked += 1;
                }
            }
        }
        assert!(checked > 50);
    }

    #[test]
    fn reads_dark_handles_translucent_colored_logos() {
        let luma = |logo: [u8; 4], bg: Color| {
            let logo_alpha = f64::from(logo[3]) / 255.0;
            let bg_alpha = f64::from(bg.a) / 255.0;
            let channel = |l: u8, b: u8| {
                let under = f64::from(b) / 255.0 * bg_alpha + (1.0 - bg_alpha);
                f64::from(l) / 255.0 * logo_alpha + under * (1.0 - logo_alpha)
            };
            0.2126 * channel(logo[0], bg.r)
                + 0.7152 * channel(logo[1], bg.g)
                + 0.0722 * channel(logo[2], bg.b)
        };
        let cell = Rect::square(Point::new(50.0, 50.0), 8.0);
        let levels = [0u8, 96, 160, 255];
        let backgrounds = [
            Color::rgb(30, 60, 250),
            Color::rgba(250, 250, 40, 150),
            Color::rgb(140, 140, 140),
        ];
        let mut checked = 0;
        for r in levels {
            for g in levels {
                for b in levels {
                    for a in [70u8, 140, 210] {
                        let image = RgbaImage::from_pixel(10, 10, Rgba([r, g, b, a]));
                        let placed = Logo::new(&image)
                            .expect("logo")
                            .with_size(LogoSize::Width(Ratio::or_one(0.1)))
                            .place(100, 100.0, 7.0, 1.0)
                            .expect("placed");
                        for bg in backgrounds {
                            let expected = luma([r, g, b, a], bg);
                            if (expected - 0.5).abs() > 0.002 {
                                assert_eq!(placed.reads_dark(cell, bg), expected < 0.5);
                                checked += 1;
                            }
                        }
                    }
                }
            }
        }
        assert!(checked > 500);
    }

    #[test]
    fn reads_dark_samples_both_sides_of_the_center() {
        let half = RgbaImage::from_fn(40, 40, |x, _| {
            if x <= 21 {
                Rgba([0, 0, 0, 255])
            } else {
                Rgba([255, 255, 255, 255])
            }
        });
        let placed = Logo::new(&half)
            .expect("logo")
            .with_size(LogoSize::Width(Ratio::or_one(0.4)))
            .place(100, 100.0, 7.0, 1.0)
            .expect("placed");
        assert!(placed.reads_dark(Rect::square(Point::new(50.0, 50.0), 16.0), Color::WHITE));
        assert!(!placed.reads_dark(Rect::square(Point::new(53.0, 50.0), 16.0), Color::WHITE));
        let rotated = RgbaImage::from_fn(40, 40, |_, y| {
            if y <= 21 {
                Rgba([0, 0, 0, 255])
            } else {
                Rgba([255, 255, 255, 255])
            }
        });
        let placed = Logo::new(&rotated)
            .expect("logo")
            .with_size(LogoSize::Width(Ratio::or_one(0.4)))
            .place(100, 100.0, 7.0, 1.0)
            .expect("placed");
        assert!(placed.reads_dark(Rect::square(Point::new(50.0, 50.0), 16.0), Color::WHITE));
    }

    #[test]
    fn heavy_downscale_keeps_large_structure() {
        let split = RgbaImage::from_fn(1600, 1600, |x, _| {
            if x < 800 {
                Rgba([0, 0, 0, 255])
            } else {
                Rgba([255, 255, 255, 255])
            }
        });
        let small = resize_premultiplied(&split, 200, 200);
        let Rgba([left, ..]) = *small.get_pixel(40, 100);
        let Rgba([right, ..]) = *small.get_pixel(160, 100);
        assert!(left < 0.01 && right > 0.99, "{left} {right}");
        let pairs = RgbaImage::from_fn(4, 1, |x, _| {
            if x < 2 {
                Rgba([255, 255, 255, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });
        let halves = box_downsample(&pairs, 2);
        assert!((halves.get_pixel(0, 0).0[3] - 1.0).abs() < 1e-12);
        assert!(halves.get_pixel(1, 0).0[3].abs() < 1e-12);
    }

    #[test]
    fn reads_dark_samples_a_three_by_three_grid() {
        let stripe = |half_width: u32| {
            RgbaImage::from_fn(40, 40, |x, _| {
                if x.abs_diff(20) < half_width || x == 20 - half_width {
                    Rgba([0, 0, 0, 255])
                } else {
                    Rgba([255, 255, 255, 255])
                }
            })
        };
        let cell = Rect::square(Point::new(50.0, 50.0), 16.0);
        let place = |image: &RgbaImage| {
            Logo::new(image)
                .expect("logo")
                .with_size(LogoSize::Width(Ratio::or_one(0.4)))
                .place(100, 100.0, 7.0, 1.0)
                .expect("placed")
        };
        assert!(!place(&stripe(2)).reads_dark(cell, Color::WHITE));
        assert!(place(&stripe(6)).reads_dark(cell, Color::WHITE));
    }

    #[test]
    fn tiny_logos_are_skipped() {
        let logo = Logo::new(&disc_logo(64))
            .expect("logo")
            .with_size(LogoSize::Width(Ratio::or_one(0.05)));
        assert!(logo.place(30, 21.0, 7.0, 1.0).is_none());
    }

    #[test]
    fn box_prefilter_averages_premultiplied_blocks() {
        let image = RgbaImage::from_fn(4, 2, |x, _| {
            if x % 2 == 0 {
                Rgba([255, 255, 255, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });
        let small = box_downsample(&image, 2);
        assert_eq!(small.dimensions(), (2, 1));
        for p in small.pixels() {
            assert!(p.0.iter().all(|v| (v - 0.5).abs() < 1e-12));
        }
        assert_eq!(box_downsample(&image, 1).dimensions(), (4, 2));
        let large = RgbaImage::from_pixel(1600, 1600, Rgba([10, 20, 30, 255]));
        let resized = resize_premultiplied(&large, 200, 200);
        assert_eq!(resized.dimensions(), (200, 200));
        let Rgba([r, _, _, a]) = *resized.get_pixel(100, 100);
        assert!((r - 10.0 / 255.0).abs() < 1e-6 && (a - 1.0).abs() < 1e-9);
    }

    #[test]
    fn premultiplied_resize_has_no_dark_fringe() {
        let image = RgbaImage::from_fn(64, 64, |x, _| {
            if x < 32 {
                Rgba([255, 255, 255, 255])
            } else {
                Rgba([0, 0, 0, 0])
            }
        });
        let resized = resize_premultiplied(&image, 16, 16);
        for p in resized.pixels() {
            let [r, g, b, a] = p.0;
            if a > 0.01 {
                assert!(
                    (r / a - 1.0).abs() < 1e-6
                        && (g / a - 1.0).abs() < 1e-6
                        && (b / a - 1.0).abs() < 1e-6
                );
            }
        }
    }

    #[test]
    fn reads_dark_composites_the_logo_over_the_background() {
        let logo = Logo::new(&disc_logo(200))
            .expect("logo")
            .with_size(LogoSize::Width(Ratio::or_one(0.2)));
        let placed = logo.place(1000, 1000.0, 70.0, 1.0).expect("placed");
        let cell = |x: f64, y: f64| Rect::square(Point::new(x, y), 10.0);
        assert!(placed.reads_dark(cell(500.0, 500.0), Color::WHITE));
        assert!(!placed.reads_dark(cell(405.0, 405.0), Color::WHITE));
        assert!(!placed.reads_dark(cell(100.0, 100.0), Color::WHITE));
        assert!(placed.reads_dark(cell(405.0, 405.0), Color::BLACK));
        assert!(!placed.reads_dark(cell(405.0, 405.0), Color::TRANSPARENT));
        let white = Logo::new(&RgbaImage::from_pixel(10, 10, Rgba([255, 255, 255, 255])))
            .expect("logo")
            .with_size(LogoSize::Width(Ratio::or_one(0.2)));
        let placed = white.place(1000, 1000.0, 70.0, 1.0).expect("placed");
        assert!(!placed.reads_dark(cell(500.0, 500.0), Color::BLACK));
    }

    #[test]
    fn footprint_distance_follows_the_logo_shape() {
        let logo = Logo::new(&disc_logo(200))
            .expect("logo")
            .with_size(LogoSize::Width(Ratio::or_one(0.2)));
        let placed = logo.place(1000, 1000.0, 70.0, 1.0).expect("placed");
        let bounds = logo
            .clone()
            .with_clearance(Clearance::Bounds)
            .place(1000, 1000.0, 70.0, 1.0)
            .expect("placed");
        let center = Point::new(500.0, 500.0);
        assert!((placed.distance(center) - 0.0).abs() < 1e-12);
        let corner = Point::new(placed.rect().min().x + 2.0, placed.rect().min().y + 2.0);
        assert!(placed.distance(corner) > 20.0);
        assert!((bounds.distance(corner) - 0.0).abs() < 1e-12);
        let far = Point::new(500.0, 700.0);
        assert!((bounds.distance(far) - 100.0).abs() < 1e-9);
        assert!(placed.distance(far) >= 100.0);
    }
}
