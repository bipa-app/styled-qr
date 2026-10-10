use std::fmt::Write as _;
use std::io::Cursor;

use base64ct::{Base64, Encoding};
use image::{DynamicImage, ImageFormat, RgbaImage};

use crate::error::Error;
use crate::geometry::{Rect, RoundedRect, Shape};
use crate::logo::unit;
use crate::raster::quantize;
use crate::scene::Scene;
use crate::style::Color;

pub(crate) fn render(scene: &Scene) -> Result<String, Error> {
    let size = scene.size();
    let mut out = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" viewBox="0 0 {size} {size}">"#
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
        let _ = write!(out, "<g{}>", fill(layer.color));
        for shape in &layer.shapes {
            out.push_str(&element(shape));
        }
        out.push_str("</g>");
    }
    if let Some(logo) = scene.logo() {
        let (w, h) = logo.image().dimensions();
        let rgba = RgbaImage::from_fn(w, h, |x, y| quantize(unit(*logo.image().get_pixel(x, y))));
        let mut png = Cursor::new(Vec::new());
        DynamicImage::ImageRgba8(rgba).write_to(&mut png, ImageFormat::Png)?;
        let (x, y) = logo.origin();
        let _ = write!(
            out,
            r#"<image x="{x}" y="{y}" width="{w}" height="{h}" href="data:image/png;base64,{}"/>"#,
            Base64::encode_string(&png.into_inner())
        );
    }
    out.push_str("</svg>");
    Ok(out)
}

fn fill(color: Color) -> String {
    let mut attrs = format!(
        r##" fill="#{:02x}{:02x}{:02x}""##,
        color.r, color.g, color.b
    );
    if color.a < u8::MAX {
        let _ = write!(
            attrs,
            r#" fill-opacity="{}""#,
            num(f64::from(color.a) / 255.0)
        );
    }
    attrs
}

fn element(shape: &Shape) -> String {
    match *shape {
        Shape::Rect(r) => format!(
            r#"<rect x="{}" y="{}" width="{}" height="{}"/>"#,
            num(r.min().x),
            num(r.min().y),
            num(r.width()),
            num(r.height())
        ),
        Shape::Disc(d) => format!(
            r#"<circle cx="{}" cy="{}" r="{}"/>"#,
            num(d.center().x),
            num(d.center().y),
            num(d.radius())
        ),
        Shape::RoundedRect(r) => format!(r#"<path d="{}"/>"#, path(r)),
        Shape::Ring(r) => format!(
            r#"<path fill-rule="evenodd" d="{}{}"/>"#,
            path(r.outer()),
            path(r.inner())
        ),
        Shape::Tile(t) => format!(r#"<path d="{}"/>"#, outline(t.rect(), t.radii())),
    }
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
    use crate::geometry::{Point, Rect};

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
    fn fill_includes_opacity_only_when_translucent() {
        assert_eq!(fill(Color::rgb(255, 0, 16)), r##" fill="#ff0010""##);
        assert_eq!(
            fill(Color::rgba(0, 0, 0, 51)),
            r##" fill="#000000" fill-opacity="0.2""##
        );
    }
}
