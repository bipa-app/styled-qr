use image::{ImageBuffer, Luma, Rgba, RgbaImage};

use crate::geometry::Shape;
use crate::logo::PremultipliedImage;
use crate::scene::{Layer, Scene};
use crate::style::Color;
use crate::units::{ceil_clamped, floor_clamped, round_clamped};

type Coverage = ImageBuffer<Luma<f64>, Vec<f64>>;

const BAND_ROWS: u32 = 32;
const STAMP_LOOKBACK: usize = 8;

struct Placement {
    x: u32,
    y: u32,
    stamp: usize,
}

struct PreparedLayer {
    color: Color,
    stamps: Vec<Coverage>,
    placements: Vec<Placement>,
}

pub(crate) fn rasterize(scene: &Scene) -> RgbaImage {
    let size = scene.size();
    let background = premultiply(scene.background());
    let layers = prepare_layers(scene.layers(), size);
    let mut out = RgbaImage::new(size, size);
    let mut out_rows = out.rows_mut();
    let mut band = PremultipliedImage::new(size, BAND_ROWS);
    let mut coverage = Coverage::new(size, BAND_ROWS);
    for top in (0..size).step_by(band_step()) {
        band.pixels_mut().for_each(|p| *p = background);
        for layer in &layers {
            for placement in &layer.placements {
                stamp_into(&mut coverage, layer, placement, top);
            }
            composite_layer(&mut band, &mut coverage, layer.color);
        }
        if let Some(logo) = scene.logo() {
            composite_image(&mut band, logo.image(), logo.origin(), top);
        }
        for (band_row, out_row) in band.rows().zip(out_rows.by_ref().take(band_step())) {
            for (src, dst) in band_row.zip(out_row) {
                *dst = quantize(*src);
            }
        }
    }
    out
}

fn band_step() -> usize {
    usize::try_from(BAND_ROWS).unwrap_or(1)
}

fn prepare_layers(layers: &[Layer], size: u32) -> Vec<PreparedLayer> {
    let mut prepared: Vec<PreparedLayer> = Vec::new();
    for layer in layers {
        if prepared.last().is_none_or(|last| last.color != layer.color) {
            prepared.push(PreparedLayer {
                color: layer.color,
                stamps: Vec::new(),
                placements: Vec::new(),
            });
        }
        if let Some(target) = prepared.last_mut() {
            let mut recent = Vec::new();
            for shape in &layer.shapes {
                target.place(shape, size, &mut recent);
            }
        }
    }
    prepared
}

impl PreparedLayer {
    fn place(&mut self, shape: &Shape, size: u32, recent: &mut Vec<(Shape, usize)>) {
        let bounds = shape.bounds();
        let x = floor_clamped(bounds.min().x, size);
        let y = floor_clamped(bounds.min().y, size);
        let local = shape.translated(-f64::from(x), -f64::from(y));
        let reused = recent
            .iter()
            .rev()
            .take(STAMP_LOOKBACK)
            .find(|(s, _)| *s == local)
            .map(|(_, i)| *i);
        let stamp = reused.unwrap_or_else(|| {
            let local_bounds = local.bounds();
            let width = ceil_clamped(local_bounds.max().x, size.saturating_sub(x));
            let height = ceil_clamped(local_bounds.max().y, size.saturating_sub(y));
            self.stamps.push(Coverage::from_fn(width, height, |px, py| {
                Luma([local.pixel_coverage(px, py)])
            }));
            recent.push((local, self.stamps.len() - 1));
            self.stamps.len() - 1
        });
        self.placements.push(Placement { x, y, stamp });
    }
}

fn stamp_into(coverage: &mut Coverage, layer: &PreparedLayer, placement: &Placement, top: u32) {
    let Some(stamp) = layer.stamps.get(placement.stamp) else {
        return;
    };
    let rows = coverage.height();
    let first = placement.y.max(top);
    let last = (placement.y + stamp.height()).min(top + rows);
    for y in first..last {
        for x in 0..stamp.width() {
            let value = stamp
                .get_pixel_checked(x, y - placement.y)
                .map_or(0.0, |&Luma([v])| v);
            if let Some(Luma([v])) = coverage
                .get_pixel_mut_checked(placement.x + x, y - top)
                .filter(|_| value > 0.0)
            {
                *v += value;
            }
        }
    }
}

fn composite_layer(canvas: &mut PremultipliedImage, coverage: &mut Coverage, color: Color) {
    let src = premultiply(color);
    for (dst, Luma([c])) in canvas.pixels_mut().zip(coverage.pixels_mut()) {
        if *c > 0.0 {
            over(dst, src, c.min(1.0));
            *c = 0.0;
        }
    }
}

fn composite_image(
    canvas: &mut PremultipliedImage,
    image: &PremultipliedImage,
    origin: (u32, u32),
    top: u32,
) {
    let rows = canvas.height();
    for (x, y, src) in image.enumerate_pixels() {
        let row = origin.1 + y;
        if (top..top + rows).contains(&row)
            && let Some(dst) = canvas.get_pixel_mut_checked(origin.0 + x, row - top)
        {
            over(dst, *src, 1.0);
        }
    }
}

fn over(dst: &mut Rgba<f64>, src: Rgba<f64>, coverage: f64) {
    let Rgba([sr, sg, sb, sa]) = src;
    let Rgba([dr, dg, db, da]) = *dst;
    let k = 1.0 - sa * coverage;
    *dst = Rgba([
        sr * coverage + dr * k,
        sg * coverage + dg * k,
        sb * coverage + db * k,
        sa * coverage + da * k,
    ]);
}

fn premultiply(color: Color) -> Rgba<f64> {
    let [r, g, b, a] = [color.r, color.g, color.b, color.a].map(|c| f64::from(c) / 255.0);
    Rgba([r * a, g * a, b * a, a])
}

pub(crate) fn quantize(Rgba([r, g, b, a]): Rgba<f64>) -> Rgba<u8> {
    let channel = |v: f64| {
        if v <= 0.0 {
            0
        } else if v >= 1.0 {
            u8::MAX
        } else {
            u8::try_from(round_clamped(v * 255.0, 255)).unwrap_or(u8::MAX)
        }
    };
    if a <= 0.0 {
        return Rgba([0; 4]);
    }
    Rgba([channel(r / a), channel(g / a), channel(b / a), channel(a)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quantize_rounds_to_nearest_and_unpremultiplies() {
        assert_eq!(quantize(Rgba([0.0, 0.0, 0.0, 0.0])), Rgba([0, 0, 0, 0]));
        assert_eq!(quantize(Rgba([1.0, 1.0, 1.0, 1.0])), Rgba([255; 4]));
        assert_eq!(
            quantize(Rgba([0.5, 0.25, 0.0, 1.0])),
            Rgba([128, 64, 0, 255])
        );
        assert_eq!(
            quantize(Rgba([0.25, 0.25, 0.25, 0.5])),
            Rgba([128, 128, 128, 128])
        );
        assert_eq!(
            quantize(Rgba([0.2 / 255.0, 0.0, 0.0, 1.0])),
            Rgba([0, 0, 0, 255])
        );
    }

    #[test]
    fn premultiply_scales_color_by_alpha() {
        let Rgba([r, g, b, a]) = premultiply(Color::rgba(255, 51, 102, 102));
        assert!((a - 0.4).abs() < 1e-12);
        assert!((r - 0.4).abs() < 1e-12);
        assert!((g - 0.08).abs() < 1e-12);
        assert!((b - 0.16).abs() < 1e-12);
    }

    #[test]
    fn over_with_partial_coverage_and_alpha() {
        let mut dst = Rgba([0.6, 0.5, 0.4, 1.0]);
        over(&mut dst, Rgba([0.2, 0.1, 0.3, 0.4]), 0.5);
        let k = 1.0 - 0.4 * 0.5;
        let expected = [0.1 + 0.6 * k, 0.05 + 0.5 * k, 0.15 + 0.4 * k, 0.2 + k];
        for (got, want) in dst.0.iter().zip(expected) {
            assert!((got - want).abs() < 1e-12, "{got} vs {want}");
        }
    }

    #[test]
    fn over_blends_premultiplied_colors() {
        let mut dst = Rgba([1.0, 1.0, 1.0, 1.0]);
        over(&mut dst, Rgba([0.0, 0.0, 0.0, 1.0]), 0.25);
        assert_eq!(dst, Rgba([0.75, 0.75, 0.75, 1.0]));
        let mut clear = Rgba([0.0; 4]);
        over(&mut clear, premultiply(Color::rgba(255, 0, 0, 51)), 1.0);
        assert!((clear.0[3] - 0.2).abs() < 1e-12 && (clear.0[0] - 0.2).abs() < 1e-12);
    }
}
