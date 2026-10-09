use std::f64::consts::{FRAC_1_SQRT_2, PI};

const CONTAINMENT_EPSILON: f64 = 1e-9;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Point {
    pub x: f64,
    pub y: f64,
}

impl Point {
    #[must_use]
    pub const fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    #[must_use]
    pub fn distance(self, other: Self) -> f64 {
        (self.x - other.x).hypot(self.y - other.y)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    min: Point,
    max: Point,
}

impl Rect {
    #[must_use]
    pub fn from_corners(a: Point, b: Point) -> Self {
        Self {
            min: Point::new(a.x.min(b.x), a.y.min(b.y)),
            max: Point::new(a.x.max(b.x), a.y.max(b.y)),
        }
    }

    #[must_use]
    pub fn from_origin(origin: Point, width: f64, height: f64) -> Self {
        Self::from_corners(origin, Point::new(origin.x + width, origin.y + height))
    }

    #[must_use]
    pub fn square(center: Point, side: f64) -> Self {
        let half = side / 2.0;
        Self::from_corners(
            Point::new(center.x - half, center.y - half),
            Point::new(center.x + half, center.y + half),
        )
    }

    #[must_use]
    pub const fn min(self) -> Point {
        self.min
    }

    #[must_use]
    pub const fn max(self) -> Point {
        self.max
    }

    #[must_use]
    pub fn width(self) -> f64 {
        self.max.x - self.min.x
    }

    #[must_use]
    pub fn height(self) -> f64 {
        self.max.y - self.min.y
    }

    #[must_use]
    pub fn center(self) -> Point {
        Point::new(
            f64::midpoint(self.min.x, self.max.x),
            f64::midpoint(self.min.y, self.max.y),
        )
    }

    #[must_use]
    pub fn area(self) -> f64 {
        self.width() * self.height()
    }

    #[must_use]
    pub fn intersection(self, other: Self) -> Option<Self> {
        let min = Point::new(self.min.x.max(other.min.x), self.min.y.max(other.min.y));
        let max = Point::new(self.max.x.min(other.max.x), self.max.y.min(other.max.y));
        (min.x < max.x && min.y < max.y).then_some(Self { min, max })
    }

    #[must_use]
    pub fn overlap_area(self, other: Self) -> f64 {
        self.intersection(other).map_or(0.0, Self::area)
    }

    #[must_use]
    pub fn inset(self, by: f64) -> Self {
        let center = self.center();
        let half_w = (self.width() / 2.0 - by).max(0.0);
        let half_h = (self.height() / 2.0 - by).max(0.0);
        Self::from_corners(
            Point::new(center.x - half_w, center.y - half_h),
            Point::new(center.x + half_w, center.y + half_h),
        )
    }

    #[must_use]
    pub fn translated(self, dx: f64, dy: f64) -> Self {
        Self {
            min: Point::new(self.min.x + dx, self.min.y + dy),
            max: Point::new(self.max.x + dx, self.max.y + dy),
        }
    }

    #[must_use]
    pub fn distance_to(self, p: Point) -> f64 {
        let dx = (self.min.x - p.x).max(p.x - self.max.x).max(0.0);
        let dy = (self.min.y - p.y).max(p.y - self.max.y).max(0.0);
        dx.hypot(dy)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Disc {
    center: Point,
    radius: f64,
}

impl Disc {
    #[must_use]
    pub fn new(center: Point, radius: f64) -> Option<Self> {
        (radius.is_finite() && radius > 0.0).then_some(Self { center, radius })
    }

    #[must_use]
    pub const fn center(self) -> Point {
        self.center
    }

    #[must_use]
    pub const fn radius(self) -> f64 {
        self.radius
    }

    fn signed_distance(self, p: Point) -> f64 {
        p.distance(self.center) - self.radius
    }

    fn area_within(self, window: Rect) -> f64 {
        disc_box_area(self.radius, self.center, window)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoundedRect {
    rect: Rect,
    radius: f64,
}

impl RoundedRect {
    #[must_use]
    pub fn new(rect: Rect, radius: f64) -> Self {
        let max = rect.width().min(rect.height()) / 2.0;
        Self {
            rect,
            radius: radius.clamp(0.0, max.max(0.0)),
        }
    }

    #[must_use]
    pub const fn rect(self) -> Rect {
        self.rect
    }

    #[must_use]
    pub const fn radius(self) -> f64 {
        self.radius
    }

    fn signed_distance(self, p: Point) -> f64 {
        let c = self.rect.center();
        let qx = (p.x - c.x).abs() - self.rect.width() / 2.0 + self.radius;
        let qy = (p.y - c.y).abs() - self.rect.height() / 2.0 + self.radius;
        qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - self.radius
    }

    fn area(self) -> f64 {
        self.rect.area() - (4.0 - PI) * self.radius * self.radius
    }

    fn area_within(self, window: Rect) -> f64 {
        let r = self.radius;
        let Point { x: x0, y: y0 } = self.rect.min;
        let Point { x: x1, y: y1 } = self.rect.max;
        let straight = [
            Rect::from_corners(Point::new(x0 + r, y0), Point::new(x1 - r, y1)),
            Rect::from_corners(Point::new(x0, y0 + r), Point::new(x0 + r, y1 - r)),
            Rect::from_corners(Point::new(x1 - r, y0 + r), Point::new(x1, y1 - r)),
        ];
        let straight_area: f64 = straight.iter().map(|s| s.overlap_area(window)).sum();
        if r <= 0.0 {
            return straight_area;
        }
        let corners = [
            (Point::new(x0 + r, y0 + r), Point::new(x0, y0)),
            (Point::new(x1 - r, y0 + r), Point::new(x1, y0)),
            (Point::new(x0 + r, y1 - r), Point::new(x0, y1)),
            (Point::new(x1 - r, y1 - r), Point::new(x1, y1)),
        ];
        let corner_area: f64 = corners
            .iter()
            .filter_map(|(center, corner)| {
                Rect::from_corners(*center, *corner)
                    .intersection(window)
                    .map(|clip| disc_box_area(r, *center, clip))
            })
            .sum();
        straight_area + corner_area
    }

    fn contains(self, inner: Self) -> bool {
        let (o, i) = (self.core(), inner.core());
        let dcx = i.center().x - o.center().x;
        let dcy = i.center().y - o.center().y;
        let dhx = (i.width() - o.width()) / 2.0;
        let dhy = (i.height() - o.height()) / 2.0;
        let dr = inner.radius - self.radius;
        [(1.0, 1.0), (1.0, -1.0), (-1.0, 1.0), (-1.0, -1.0)]
            .iter()
            .all(|(sx, sy)| {
                let a = sx * dcx + dhx;
                let b = sy * dcy + dhy;
                let support = if a >= 0.0 && b >= 0.0 {
                    a.hypot(b)
                } else {
                    a.max(b)
                };
                support + dr <= CONTAINMENT_EPSILON
            })
    }

    fn core(self) -> Rect {
        self.rect.inset(self.radius)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ring {
    outer: RoundedRect,
    inner: RoundedRect,
}

impl Ring {
    #[must_use]
    pub fn new(outer: RoundedRect, inner: RoundedRect) -> Option<Self> {
        outer.contains(inner).then_some(Self { outer, inner })
    }

    #[must_use]
    pub const fn outer(self) -> RoundedRect {
        self.outer
    }

    #[must_use]
    pub const fn inner(self) -> RoundedRect {
        self.inner
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RectCorner {
    TopLeft,
    TopRight,
    BottomRight,
    BottomLeft,
}

impl RectCorner {
    pub const ALL: [Self; 4] = [
        Self::TopLeft,
        Self::TopRight,
        Self::BottomRight,
        Self::BottomLeft,
    ];

    const fn bit(self) -> u8 {
        match self {
            Self::TopLeft => 1,
            Self::TopRight => 2,
            Self::BottomRight => 4,
            Self::BottomLeft => 8,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Corners(u8);

impl Corners {
    pub const NONE: Self = Self(0);
    pub const ALL: Self = Self(15);

    #[must_use]
    pub const fn with(self, corner: RectCorner) -> Self {
        Self(self.0 | corner.bit())
    }

    #[must_use]
    pub const fn contains(self, corner: RectCorner) -> bool {
        self.0 & corner.bit() != 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tile {
    rect: Rect,
    radius: f64,
    rounded: Corners,
}

impl Tile {
    #[must_use]
    pub fn new(rect: Rect, radius: f64, rounded: Corners) -> Self {
        Self {
            rect,
            radius: RoundedRect::new(rect, radius).radius,
            rounded,
        }
    }

    #[must_use]
    pub const fn rect(self) -> Rect {
        self.rect
    }

    #[must_use]
    pub const fn rounded(self) -> Corners {
        self.rounded
    }

    #[must_use]
    pub fn radii(self) -> [f64; 4] {
        RectCorner::ALL.map(|corner| {
            if self.rounded.contains(corner) {
                self.radius
            } else {
                0.0
            }
        })
    }

    fn corner_pieces(self) -> impl Iterator<Item = (Point, Rect)> {
        let r = self.radius;
        let Point { x: x0, y: y0 } = self.rect.min;
        let Point { x: x1, y: y1 } = self.rect.max;
        let pieces = [
            (Point::new(x0 + r, y0 + r), Point::new(x0, y0)),
            (Point::new(x1 - r, y0 + r), Point::new(x1, y0)),
            (Point::new(x1 - r, y1 - r), Point::new(x1, y1)),
            (Point::new(x0 + r, y1 - r), Point::new(x0, y1)),
        ];
        let rounded = self.rounded;
        pieces
            .into_iter()
            .zip(RectCorner::ALL)
            .filter(move |(_, corner)| rounded.contains(*corner) && r > 0.0)
            .map(|((center, corner), _)| (center, Rect::from_corners(center, corner)))
    }

    fn signed_distance(self, p: Point) -> f64 {
        let c = self.rect.center();
        let corner = match (p.x >= c.x, p.y >= c.y) {
            (false, false) => RectCorner::TopLeft,
            (true, false) => RectCorner::TopRight,
            (true, true) => RectCorner::BottomRight,
            (false, true) => RectCorner::BottomLeft,
        };
        let radius = if self.rounded.contains(corner) {
            self.radius
        } else {
            0.0
        };
        RoundedRect::new(self.rect, radius).signed_distance(p)
    }

    fn area(self) -> f64 {
        let corner = (1.0 - PI / 4.0) * self.radius * self.radius;
        self.rect.area() - self.corner_pieces().map(|_| corner).sum::<f64>()
    }

    fn area_within(self, window: Rect) -> f64 {
        let cut: f64 = self
            .corner_pieces()
            .filter_map(|(center, corner)| corner.intersection(window).map(|clip| (center, clip)))
            .map(|(center, clip)| clip.area() - disc_box_area(self.radius, center, clip))
            .sum();
        (self.rect.overlap_area(window) - cut).max(0.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    Rect(Rect),
    Disc(Disc),
    RoundedRect(RoundedRect),
    Ring(Ring),
    Tile(Tile),
}

impl Shape {
    #[must_use]
    pub fn translated(&self, dx: f64, dy: f64) -> Self {
        let rounded = |r: RoundedRect| RoundedRect {
            rect: r.rect.translated(dx, dy),
            radius: r.radius,
        };
        match *self {
            Self::Rect(r) => Self::Rect(r.translated(dx, dy)),
            Self::Disc(d) => Self::Disc(Disc {
                center: Point::new(d.center.x + dx, d.center.y + dy),
                radius: d.radius,
            }),
            Self::RoundedRect(r) => Self::RoundedRect(rounded(r)),
            Self::Ring(r) => Self::Ring(Ring {
                outer: rounded(r.outer),
                inner: rounded(r.inner),
            }),
            Self::Tile(t) => Self::Tile(Tile {
                rect: t.rect.translated(dx, dy),
                ..t
            }),
        }
    }

    #[must_use]
    pub fn bounds(&self) -> Rect {
        match *self {
            Self::Rect(r) => r,
            Self::Disc(d) => Rect::square(d.center, 2.0 * d.radius),
            Self::RoundedRect(r) => r.rect,
            Self::Ring(r) => r.outer.rect,
            Self::Tile(t) => t.rect,
        }
    }

    #[must_use]
    pub fn area(&self) -> f64 {
        match *self {
            Self::Rect(r) => r.area(),
            Self::Disc(d) => PI * d.radius * d.radius,
            Self::RoundedRect(r) => r.area(),
            Self::Ring(r) => r.outer.area() - r.inner.area(),
            Self::Tile(t) => t.area(),
        }
    }

    #[must_use]
    pub fn signed_distance(&self, p: Point) -> f64 {
        match *self {
            Self::Rect(r) => RoundedRect::new(r, 0.0).signed_distance(p),
            Self::Disc(d) => d.signed_distance(p),
            Self::RoundedRect(r) => r.signed_distance(p),
            Self::Ring(r) => r.outer.signed_distance(p).max(-r.inner.signed_distance(p)),
            Self::Tile(t) => t.signed_distance(p),
        }
    }

    #[must_use]
    pub fn area_within(&self, window: Rect) -> f64 {
        match *self {
            Self::Rect(r) => r.overlap_area(window),
            Self::Disc(d) => d.area_within(window),
            Self::RoundedRect(r) => r.area_within(window),
            Self::Ring(r) => (r.outer.area_within(window) - r.inner.area_within(window)).max(0.0),
            Self::Tile(t) => t.area_within(window),
        }
    }

    #[must_use]
    pub fn pixel_coverage(&self, x: u32, y: u32) -> f64 {
        let pixel = Rect::from_origin(Point::new(f64::from(x), f64::from(y)), 1.0, 1.0);
        let distance = self.signed_distance(pixel.center());
        if distance >= FRAC_1_SQRT_2 {
            0.0
        } else if distance <= -FRAC_1_SQRT_2 {
            1.0
        } else {
            self.area_within(pixel).clamp(0.0, 1.0)
        }
    }
}

fn disc_box_area(r: f64, center: Point, window: Rect) -> f64 {
    let x0 = window.min.x - center.x;
    let x1 = window.max.x - center.x;
    let y0 = window.min.y - center.y;
    let y1 = window.max.y - center.y;
    let area = signed_quadrant_area(r, x1, y1)
        - signed_quadrant_area(r, x0, y1)
        - signed_quadrant_area(r, x1, y0)
        + signed_quadrant_area(r, x0, y0);
    area.max(0.0)
}

fn signed_quadrant_area(r: f64, x: f64, y: f64) -> f64 {
    x.signum() * y.signum() * quadrant_area(r, x.abs(), y.abs())
}

fn quadrant_area(r: f64, x: f64, y: f64) -> f64 {
    let x = x.min(r);
    let y = y.min(r);
    if x * x + y * y <= r * r {
        return x * y;
    }
    let a = (r * r - y * y).max(0.0).sqrt();
    a * y + circle_primitive(r, x) - circle_primitive(r, a)
}

fn circle_primitive(r: f64, u: f64) -> f64 {
    let u = u.clamp(-r, r);
    let half_chord = ((r - u) * (r + u)).max(0.0).sqrt();
    f64::midpoint(u * half_chord, r * r * u.atan2(half_chord))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f64, b: f64, eps: f64) -> bool {
        (a - b).abs() <= eps
    }

    fn supersampled(shape: &Shape, x: u32, y: u32) -> f64 {
        let n = 64;
        let mut inside = 0u32;
        for j in 0..n {
            for i in 0..n {
                let p = Point::new(
                    f64::from(x) + (f64::from(i) + 0.5) / f64::from(n),
                    f64::from(y) + (f64::from(j) + 0.5) / f64::from(n),
                );
                if shape.signed_distance(p) < 0.0 {
                    inside += 1;
                }
            }
        }
        f64::from(inside) / f64::from(n * n)
    }

    fn total_coverage(shape: &Shape, size: u32) -> f64 {
        (0..size)
            .flat_map(|y| (0..size).map(move |x| (x, y)))
            .map(|(x, y)| shape.pixel_coverage(x, y))
            .sum()
    }

    fn shapes() -> Vec<Shape> {
        let rr = RoundedRect::new(
            Rect::from_corners(Point::new(2.3, 3.1), Point::new(17.9, 14.2)),
            3.7,
        );
        let inner = RoundedRect::new(
            Rect::from_corners(Point::new(5.0, 6.0), Point::new(15.0, 11.5)),
            1.5,
        );
        vec![
            Shape::Disc(Disc::new(Point::new(9.37, 8.81), 6.13).expect("disc")),
            Shape::Disc(Disc::new(Point::new(10.0, 10.0), 0.4).expect("disc")),
            Shape::Rect(Rect::from_corners(
                Point::new(1.25, 2.5),
                Point::new(7.75, 9.1),
            )),
            Shape::RoundedRect(rr),
            Shape::Ring(Ring::new(rr, inner).expect("ring")),
            Shape::Tile(Tile::new(
                Rect::from_corners(Point::new(3.5, 2.25), Point::new(15.5, 17.0)),
                4.2,
                Corners::NONE
                    .with(RectCorner::TopLeft)
                    .with(RectCorner::BottomRight),
            )),
            Shape::Tile(Tile::new(
                Rect::from_corners(Point::new(1.0, 1.0), Point::new(9.0, 9.0)),
                99.0,
                Corners::ALL,
            )),
        ]
    }

    #[test]
    fn exact_coverage_matches_supersampling() {
        for shape in shapes() {
            for y in 0..20 {
                for x in 0..20 {
                    let exact = shape.pixel_coverage(x, y);
                    let sampled = supersampled(&shape, x, y);
                    assert!(
                        close(exact, sampled, 0.01),
                        "{shape:?} at ({x},{y}): {exact} vs {sampled}"
                    );
                }
            }
        }
    }

    #[test]
    fn translation_by_whole_pixels_shifts_coverage_exactly() {
        for shape in shapes() {
            let moved = shape.translated(3.0, 5.0);
            assert!((moved.area() - shape.area()).abs() < 1e-12);
            assert_eq!(moved.bounds(), shape.bounds().translated(3.0, 5.0));
            for y in 0..20 {
                for x in 0..20 {
                    let a = shape.pixel_coverage(x, y);
                    let b = moved.pixel_coverage(x + 3, y + 5);
                    assert!((a - b).abs() < 1e-12, "{shape:?} ({x},{y})");
                }
            }
        }
    }

    #[test]
    fn coverage_integrates_to_area() {
        for shape in shapes() {
            let total = total_coverage(&shape, 24);
            assert!(
                close(total, shape.area(), 1e-6),
                "{shape:?}: {total} vs {}",
                shape.area()
            );
        }
    }

    #[test]
    fn disc_coverage_is_symmetric_about_its_center() {
        let disc = Shape::Disc(Disc::new(Point::new(11.0, 11.0), 8.8).expect("disc"));
        for y in 0..22 {
            for x in 0..22 {
                let c = disc.pixel_coverage(x, y);
                assert!(close(c, disc.pixel_coverage(21 - x, y), 1e-12));
                assert!(close(c, disc.pixel_coverage(x, 21 - y), 1e-12));
                assert!(close(c, disc.pixel_coverage(y, x), 1e-12));
            }
        }
    }

    #[test]
    fn disc_coverage_keeps_precision_at_the_disc_ends() {
        let disc = Shape::Disc(Disc::new(Point::new(7.7, 6.3), 4.7).expect("disc"));
        let moved = disc.translated(3.0, 5.0);
        for y in 0..14 {
            for x in 0..14 {
                let a = disc.pixel_coverage(x, y);
                let b = moved.pixel_coverage(x + 3, y + 5);
                assert!((a - b).abs() < 1e-12, "({x},{y}) {a} {b}");
            }
        }
        let r = 2.5;
        for step in 0..=100u32 {
            let u = -r + f64::from(step) * 0.05;
            let expected =
                f64::midpoint(u * (r * r - u * u).max(0.0).sqrt(), r * r * (u / r).asin());
            assert!((circle_primitive(r, u) - expected).abs() < 1e-9);
        }
    }

    #[test]
    fn quadrant_area_edge_cases() {
        let r = 3.0;
        assert!(close(quadrant_area(r, r, r), PI * r * r / 4.0, 1e-12));
        assert!(close(quadrant_area(r, 10.0, 10.0), PI * r * r / 4.0, 1e-12));
        assert!(close(quadrant_area(r, 1.0, 1.0), 1.0, 1e-12));
        assert!(close(quadrant_area(r, 0.0, 2.0), 0.0, 1e-12));
        assert!(close(quadrant_area(r, r, 0.5), 0.5 * r, 0.05));
    }

    #[test]
    fn tiles_match_rounded_rects_and_rects_at_the_extremes() {
        let rect = Rect::from_corners(Point::new(2.0, 3.0), Point::new(13.0, 14.0));
        let all = Shape::Tile(Tile::new(rect, 3.0, Corners::ALL));
        let rounded = Shape::RoundedRect(RoundedRect::new(rect, 3.0));
        let none = Shape::Tile(Tile::new(rect, 3.0, Corners::NONE));
        for y in 0..16 {
            for x in 0..16 {
                assert!((all.pixel_coverage(x, y) - rounded.pixel_coverage(x, y)).abs() < 1e-12);
                assert!(
                    (none.pixel_coverage(x, y) - Shape::Rect(rect).pixel_coverage(x, y)).abs()
                        < 1e-12
                );
            }
        }
        let corners = Corners::NONE
            .with(RectCorner::TopRight)
            .with(RectCorner::BottomLeft);
        let tile = Tile::new(rect, 3.0, corners);
        let same = |a: [f64; 4], b: [f64; 4]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-12);
        assert!(same(tile.radii(), [0.0, 3.0, 0.0, 3.0]));
        assert_eq!(tile.rounded(), corners);
        assert!(corners.contains(RectCorner::TopRight) && !corners.contains(RectCorner::TopLeft));
        assert!(same(Tile::new(rect, 50.0, Corners::ALL).radii(), [5.5; 4]));
        assert!(RectCorner::ALL.iter().all(|c| Corners::ALL.contains(*c)));
        assert!(RectCorner::ALL.iter().all(|c| !Corners::NONE.contains(*c)));
    }

    #[test]
    fn rounded_rect_radius_is_clamped() {
        let rect = Rect::from_corners(Point::new(0.0, 0.0), Point::new(4.0, 10.0));
        assert!((RoundedRect::new(rect, 7.0).radius() - 2.0).abs() < 1e-12);
        assert!((RoundedRect::new(rect, -1.0).radius() - 0.0).abs() < 1e-12);
        let circle = Shape::RoundedRect(RoundedRect::new(
            Rect::from_corners(Point::new(0.0, 0.0), Point::new(4.0, 4.0)),
            2.0,
        ));
        assert!(close(circle.area(), PI * 4.0, 1e-12));
    }

    #[test]
    fn ring_requires_containment() {
        let outer = RoundedRect::new(
            Rect::from_corners(Point::new(0.0, 0.0), Point::new(10.0, 10.0)),
            3.0,
        );
        let centered = RoundedRect::new(Rect::square(Point::new(5.0, 5.0), 6.0), 1.0);
        assert!(Ring::new(outer, centered).is_some());
        let poking_out = RoundedRect::new(Rect::square(Point::new(5.0, 5.0), 10.0), 0.0);
        assert!(Ring::new(outer, poking_out).is_none());
        let corner = RoundedRect::new(
            Rect::from_corners(Point::new(0.2, 0.2), Point::new(2.0, 2.0)),
            0.0,
        );
        assert!(Ring::new(outer, corner).is_none());
        for (x, y) in [
            (9.0, 5.0),
            (1.0, 5.0),
            (5.0, 1.0),
            (5.0, 9.0),
            (8.5, 1.5),
            (1.5, 8.5),
        ] {
            let shifted = RoundedRect::new(Rect::square(Point::new(x, y), 4.0), 1.0);
            assert!(Ring::new(outer, shifted).is_none(), "({x},{y})");
        }
        for (x, y) in [(3.5, 5.0), (6.5, 5.0), (5.0, 3.5), (5.0, 6.5)] {
            let inside = RoundedRect::new(Rect::square(Point::new(x, y), 4.0), 1.0);
            assert!(Ring::new(outer, inside).is_some(), "({x},{y})");
        }
        let tall = RoundedRect::new(
            Rect::from_corners(Point::new(4.0, 0.5), Point::new(6.0, 9.5)),
            1.0,
        );
        assert!(Ring::new(outer, tall).is_some());
        let too_tall = RoundedRect::new(
            Rect::from_corners(Point::new(4.0, -0.5), Point::new(6.0, 9.5)),
            1.0,
        );
        assert!(Ring::new(outer, too_tall).is_none());
        for (x, y) in [(7.5, 2.5), (2.5, 7.5), (2.5, 2.5), (7.5, 7.5)] {
            let corner_poke = RoundedRect::new(Rect::square(Point::new(x, y), 4.0), 0.0);
            assert!(Ring::new(outer, corner_poke).is_none(), "({x},{y})");
            let rounded = RoundedRect::new(Rect::square(Point::new(x, y), 4.0), 2.0);
            assert!(Ring::new(outer, rounded).is_some(), "({x},{y})");
        }
        assert!(Ring::new(outer, outer).is_some());
    }

    #[test]
    fn rect_helpers() {
        let r = Rect::from_corners(Point::new(4.0, 6.0), Point::new(0.0, 2.0));
        assert_eq!(r.min(), Point::new(0.0, 2.0));
        assert_eq!(r.max(), Point::new(4.0, 6.0));
        assert_eq!(r.center(), Point::new(2.0, 4.0));
        assert!((r.distance_to(Point::new(2.0, 4.0)) - 0.0).abs() < 1e-12);
        assert!((r.distance_to(Point::new(7.0, 10.0)) - 5.0).abs() < 1e-12);
        assert_eq!(
            r.inset(1.0),
            Rect::from_corners(Point::new(1.0, 3.0), Point::new(3.0, 5.0))
        );
        assert!((r.inset(5.0).area() - 0.0).abs() < 1e-12);
        assert!(
            r.intersection(Rect::square(Point::new(10.0, 10.0), 1.0))
                .is_none()
        );
        let touching = Rect::from_corners(Point::new(4.0, 2.0), Point::new(6.0, 6.0));
        assert!(r.intersection(touching).is_none());
        let flat = Rect::from_corners(Point::new(0.0, 6.0), Point::new(4.0, 8.0));
        assert!(r.intersection(flat).is_none());
        assert!((r.overlap_area(Rect::square(Point::new(4.0, 6.0), 2.0)) - 1.0).abs() < 1e-12);
        assert!(Disc::new(Point::new(0.0, 0.0), 0.0).is_none());
        assert!(Disc::new(Point::new(0.0, 0.0), f64::NAN).is_none());
    }
}
