use std::fmt::Write as _;
use std::io::Cursor;

use base64ct::{Base64, Encoding};
use image::{DynamicImage, ImageFormat, RgbaImage};

use crate::error::Error;
use crate::geometry::{Rect, RoundedRect, Shape};
use crate::logo::unit;
use crate::raster::quantize;
use crate::scene::Scene;
use crate::style::{Blending, Color};

pub(crate) fn render(scene: &Scene) -> Result<String, Error> {
    let size = scene.size();
    let interpolation = match scene.blending() {
        Blending::Srgb => "",
        Blending::Linear => r#" color-interpolation="linearRGB""#,
    };
    let mut out = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" width="{size}" height="{size}" viewBox="0 0 {size} {size}"{interpolation}>"#
    );
    let background = scene.background();
    if background.a > 0 {
        let _ = write!(
            out,
            r#"<rect width="{size}" height="{size}"{}/>"#,
            fill(background)
        );
    }
    for layer in scene.layers().iter().filter(|l| !l.shapes.is_empty()) {
        out.push_str(&group(&layer.shapes, layer.color));
    }
    if let Some(logo) = scene.logo() {
        let (w, h) = logo.image().dimensions();
        let rgba = RgbaImage::from_fn(w, h, |x, y| quantize(unit(*logo.image().get_pixel(x, y))));
        let mut png = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(rgba).write_to(&mut png, ImageFormat::Png)?;
        let (x, y) = logo.origin();
        let _ = write!(
            out,
            r#"<image x="{x}" y="{y}" width="{w}" height="{h}" xlink:href="data:image/png;base64,{}"/>"#,
            Base64::encode_string(&png.into_inner())
        );
    }
    out.push_str("</svg>");
    Ok(out)
}

fn fill(color: Color) -> String {
    paint("fill", color)
}

fn paint(property: &str, color: Color) -> String {
    let mut attrs = format!(
        r##" {property}="#{:02x}{:02x}{:02x}""##,
        color.r, color.g, color.b
    );
    if color.a < u8::MAX {
        let _ = write!(
            attrs,
            r#" {property}-opacity="{}""#,
            num(f64::from(color.a) / 255.0)
        );
    }
    attrs
}

/// One layer in as few elements as it can take: its solid shapes as one path (all wound
/// clockwise, so overlaps stay filled), its rings each with their own even-odd rule, and its
/// dots of one radius as zero-length round-capped strokes.
fn group(shapes: &[Shape], color: Color) -> String {
    let mut solid = String::new();
    let mut rings = String::new();
    let mut dots: Vec<(String, String)> = Vec::new();
    for shape in shapes {
        match *shape {
            Shape::Rect(r) => solid.push_str(&outline(r, [0.0; 4])),
            Shape::RoundedRect(r) => solid.push_str(&path(r)),
            Shape::Tile(t) => solid.push_str(&outline(t.rect(), t.radii())),
            Shape::Ring(r) => {
                let _ = write!(
                    rings,
                    r#"<path fill-rule="evenodd" d="{}{}"/>"#,
                    path(r.outer()),
                    path(r.inner())
                );
            }
            Shape::Disc(d) => {
                let width = num(2.0 * d.radius());
                let at = format!("M{} {}h0", num(d.center().x), num(d.center().y));
                match dots.iter_mut().find(|(w, _)| *w == width) {
                    Some((_, d)) => d.push_str(&at),
                    None => dots.push((width, at)),
                }
            }
        }
    }
    let mut out = format!("<g{}>", fill(color));
    if !solid.is_empty() {
        let _ = write!(out, r#"<path d="{solid}"/>"#);
    }
    out.push_str(&rings);
    for (width, d) in dots {
        let _ = write!(
            out,
            r#"<path fill="none"{} stroke-width="{width}" stroke-linecap="round" d="{d}"/>"#,
            paint("stroke", color)
        );
    }
    out.push_str("</g>");
    out
}

fn path(rr: RoundedRect) -> String {
    outline(rr.rect(), [rr.radius(); 4])
}

fn outline(rect: Rect, [tl, tr, br, bl]: [f64; 4]) -> String {
    let (min, max) = (rect.min(), rect.max());
    let [x0, y0, x1, y1] = [min.x, min.y, max.x, max.y];
    if [tl, tr, br, bl].iter().all(|r| *r <= 0.0) {
        return format!(
            "M{} {}H{}V{}H{}Z",
            num(x0),
            num(y0),
            num(x1),
            num(y1),
            num(x0)
        );
    }
    let arc = |r: f64, x: f64, y: f64| {
        if r > 0.0 {
            format!("A{} {} 0 0 1 {} {}", num(r), num(r), num(x), num(y))
        } else {
            String::new()
        }
    };
    format!(
        "M{} {}H{}{}V{}{}H{}{}V{}{}Z",
        num(x0 + tl),
        num(y0),
        num(x1 - tr),
        arc(tr, x1, y0 + tr),
        num(y1 - br),
        arc(br, x1 - br, y1),
        num(x0 + bl),
        arc(bl, x0, y1 - bl),
        num(y0 + tl),
        arc(tl, x0 + tl, y0),
    )
}

fn num(v: f64) -> String {
    let s = format!("{v:.4}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" {
        "0".to_owned()
    } else {
        s.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geometry::{Disc, Point, Rect};

    #[test]
    fn numbers_are_compact() {
        assert_eq!(num(1.0), "1");
        assert_eq!(num(1.5), "1.5");
        assert_eq!(num(2.123_456), "2.1235");
        assert_eq!(num(-0.000_01), "0");
        assert_eq!(num(0.0), "0");
    }

    #[test]
    fn rounded_rect_path_closes_with_four_arcs() {
        let rr = RoundedRect::new(
            Rect::from_corners(Point::new(0.0, 0.0), Point::new(10.0, 6.0)),
            2.0,
        );
        assert_eq!(
            path(rr),
            "M2 0H8A2 2 0 0 1 10 2V4A2 2 0 0 1 8 6H2A2 2 0 0 1 0 4V2A2 2 0 0 1 2 0Z"
        );
        let square = RoundedRect::new(
            Rect::from_corners(Point::new(1.0, 1.0), Point::new(3.0, 3.0)),
            0.0,
        );
        assert_eq!(path(square), "M1 1H3V3H1Z");
        let rect = Rect::from_corners(Point::new(0.0, 0.0), Point::new(10.0, 6.0));
        assert_eq!(
            outline(rect, [0.0, 2.0, 0.0, 1.0]),
            "M0 0H8A2 2 0 0 1 10 2V6H1A1 1 0 0 1 0 5V0Z"
        );
    }

    #[test]
    fn a_layer_is_one_solid_path_its_rings_and_one_stroke_per_dot_size() {
        let rect = |x: f64| Rect::from_corners(Point::new(x, 0.0), Point::new(x + 2.0, 2.0));
        let shapes = [
            Shape::Rect(rect(0.0)),
            Shape::Rect(rect(2.0)),
            Shape::Disc(Disc::new(Point::new(5.0, 1.0), 1.0).expect("disc")),
            Shape::Disc(Disc::new(Point::new(7.5, 1.0), 1.0).expect("disc")),
            Shape::Disc(Disc::new(Point::new(9.0, 1.0), 0.5).expect("disc")),
        ];
        assert_eq!(
            group(&shapes, Color::rgba(0, 0, 0, 51)),
            concat!(
                r##"<g fill="#000000" fill-opacity="0.2"><path d="M0 0H2V2H0ZM2 0H4V2H2Z"/>"##,
                r##"<path fill="none" stroke="#000000" stroke-opacity="0.2" stroke-width="2" stroke-linecap="round" d="M5 1h0M7.5 1h0"/>"##,
                r##"<path fill="none" stroke="#000000" stroke-opacity="0.2" stroke-width="1" stroke-linecap="round" d="M9 1h0"/></g>"##,
            )
        );
    }

    #[test]
    fn fill_includes_opacity_only_when_translucent() {
        assert_eq!(fill(Color::rgb(255, 0, 16)), r##" fill="#ff0010""##);
        assert_eq!(
            fill(Color::rgba(0, 0, 0, 51)),
            r##" fill="#000000" fill-opacity="0.2""##
        );
    }
}
