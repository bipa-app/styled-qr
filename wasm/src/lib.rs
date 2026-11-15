//! `render` draws a QR code from a person's choices: a shape, the finder corners, the colors and
//! an optional logo. Every refusal carries a stable code, so an editor can say what to change.

use serde::{Deserialize, Serialize};
use styled_qr::{
    AlignmentStyle, Color, Composer, EcLevel, Error, Layout, Logo, LogoSize, ModuleShape,
    ModuleSize, Padding, Palette, PatternShape, Plate, QuietZone, Ratio, Roundness, Style, Symbol,
};
use wasm_bindgen::prelude::*;

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Shape {
    #[default]
    Dots,
    Rounded,
    Connected,
    Square,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Finder {
    #[default]
    Rounded,
    Square,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
pub enum Ec {
    L,
    M,
    Q,
    #[default]
    H,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Colors {
    /// `#rrggbb`.
    pub dark: String,
    pub light: String,
    /// The finder patterns' color; the dark color when absent.
    pub finder: Option<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlateOptions {
    pub color: String,
    pub margin: f64,
    pub roundness: f64,
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LogoOptions {
    /// The logo's width as a share of the code's; the library's default area when absent.
    pub width: Option<f64>,
    /// Empty modules around the logo.
    #[serde(default = "default_logo_padding")]
    pub padding: u32,
    pub plate: Option<PlateOptions>,
    /// Link the SVG's logo from this URL instead of embedding it; the PNG still embeds it.
    pub href: Option<String>,
}

const fn default_logo_padding() -> u32 {
    2
}

#[derive(Clone, Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Options {
    #[serde(default)]
    pub ec: Ec,
    #[serde(default = "default_module_px")]
    pub module_px: u32,
    #[serde(default = "default_quiet_zone")]
    pub quiet_zone: u32,
    #[serde(default)]
    pub shape: Shape,
    #[serde(default)]
    pub finder: Finder,
    pub colors: Option<Colors>,
    /// Applies when `render` receives a logo.
    pub logo: Option<LogoOptions>,
    /// Also return the PNG; the SVG always comes.
    #[serde(default)]
    pub png: bool,
}

const fn default_module_px() -> u32 {
    11
}

const fn default_quiet_zone() -> u32 {
    2
}

/// Why a code wasn't drawn: `code` is stable, `message` is for logs.
#[derive(Debug, Serialize, PartialEq)]
pub struct Refusal {
    pub code: &'static str,
    pub message: String,
}

impl Refusal {
    fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl From<Error> for Refusal {
    fn from(error: Error) -> Self {
        let code = match &error {
            Error::Encode(_) | Error::Version(_) => "data",
            Error::InsufficientContrast { .. } => "contrast",
            Error::LogoHidesFunctionPattern(_) | Error::LogoExceedsErrorBudget { .. } => {
                "logoUnreadable"
            }
            Error::EmptyLogo | Error::LogoDoesNotFit { .. } | Error::Image(_) => "logoImage",
            Error::CanvasTooLarge { .. } => "tooLarge",
            _ => "options",
        };
        Self::new(code, error.to_string())
    }
}

#[derive(Debug)]
pub struct Drawn {
    pub svg: String,
    pub png: Option<Vec<u8>>,
}

fn color(hex: &str) -> Result<Color, Refusal> {
    let digits = hex.strip_prefix('#').unwrap_or(hex);
    let channel = |at: usize| {
        digits
            .get(at..at + 2)
            .and_then(|pair| u8::from_str_radix(pair, 16).ok())
    };
    match (digits.len(), channel(0), channel(2), channel(4)) {
        (6, Some(r), Some(g), Some(b)) => Ok(Color::rgb(r, g, b)),
        _ => Err(Refusal::new("color", format!("{hex} is not #rrggbb"))),
    }
}

fn style(options: &Options) -> Result<Style, Refusal> {
    let module = match options.shape {
        Shape::Dots => ModuleShape::DOT,
        Shape::Rounded => ModuleShape::Rounded {
            size: Ratio::new(0.9)?,
            roundness: Roundness::new(0.45)?,
        },
        Shape::Connected => ModuleShape::Connected {
            roundness: Roundness::new(0.5)?,
        },
        Shape::Square => ModuleShape::Square,
    };
    let finder = match options.finder {
        Finder::Rounded => PatternShape::ROUNDED_FINDER,
        Finder::Square => PatternShape::SQUARE_FINDER,
    };
    let palette = match &options.colors {
        None => Style::DOTS.palette,
        Some(colors) => {
            let dark = color(&colors.dark)?;
            let palette = Palette::new(dark, color(&colors.light)?)?;
            match &colors.finder {
                Some(finder) => palette.with_finder(color(finder)?)?,
                None => palette,
            }
        }
    };
    Ok(Style {
        module,
        finder,
        alignment: AlignmentStyle::Modules,
        palette,
        ..Style::DOTS
    })
}

fn logo(png: &[u8], options: Option<&LogoOptions>) -> Result<Logo, Refusal> {
    let image = image::load_from_memory_with_format(png, image::ImageFormat::Png)
        .map_err(|error| Refusal::new("logoImage", error.to_string()))?;
    let mut logo = Logo::new(&image.into_rgba8())?;
    let options = options.cloned().unwrap_or(LogoOptions {
        width: None,
        padding: default_logo_padding(),
        plate: None,
        href: None,
    });
    if let Some(width) = options.width {
        logo = logo.with_size(LogoSize::Width(Ratio::new(width)?));
    }
    if let Some(plate) = &options.plate {
        logo = logo.with_plate(Plate {
            color: color(&plate.color)?,
            margin: Ratio::new(plate.margin)?,
            roundness: Roundness::new(plate.roundness)?,
        });
    }
    Ok(logo.with_padding(Padding::new(options.padding)?))
}

/// The Rust entry point, which the tests drive directly.
pub fn draw(data: &str, options: &Options, logo_png: Option<&[u8]>) -> Result<Drawn, Refusal> {
    let ec = match options.ec {
        Ec::L => EcLevel::L,
        Ec::M => EcLevel::M,
        Ec::Q => EcLevel::Q,
        Ec::H => EcLevel::H,
    };
    let symbol = Symbol::encode(data.as_bytes(), ec)?;
    let layout = Layout::new(
        ModuleSize::new(options.module_px)?,
        QuietZone::new(options.quiet_zone)?,
    );
    let style = style(options)?;
    let logo = logo_png
        .map(|png| logo(png, options.logo.as_ref()))
        .transpose()?;
    let composer = Composer::new(&symbol, layout).style(style);
    let scene = match &logo {
        Some(logo) => composer.logo(logo).compose()?,
        None => composer.compose()?,
    };
    let href = logo
        .as_ref()
        .and(options.logo.as_ref())
        .and_then(|logo| logo.href.as_deref());
    Ok(Drawn {
        svg: match href {
            Some(href) => scene.to_svg_linking_logo(href)?,
            None => scene.to_svg()?,
        },
        png: if options.png {
            Some(scene.to_png()?)
        } else {
            None
        },
    })
}

#[wasm_bindgen(getter_with_clone)]
pub struct Rendered {
    pub svg: String,
    pub png: Option<Vec<u8>>,
}

/// Draws `data` as a QR code. Throws `{ code, message }` when the choices can't make a code that
/// reads: `contrast`, `logoUnreadable`, `logoImage`, `color`, `data`, `tooLarge` or `options`.
#[wasm_bindgen]
pub fn render(
    data: &str,
    options: JsValue,
    logo_png: Option<Vec<u8>>,
) -> Result<Rendered, JsValue> {
    let refuse = |refusal: Refusal| {
        serde_wasm_bindgen::to_value(&refusal).unwrap_or_else(|_| JsValue::from_str(refusal.code))
    };
    let options: Options = if options.is_undefined() || options.is_null() {
        defaults()
    } else {
        serde_wasm_bindgen::from_value(options)
            .map_err(|error| refuse(Refusal::new("options", error.to_string())))?
    };
    draw(data, &options, logo_png.as_deref())
        .map(|drawn| Rendered {
            svg: drawn.svg,
            png: drawn.png,
        })
        .map_err(refuse)
}

fn defaults() -> Options {
    Options {
        ec: Ec::default(),
        module_px: default_module_px(),
        quiet_zone: default_quiet_zone(),
        shape: Shape::default(),
        finder: Finder::default(),
        colors: None,
        logo: None,
        png: false,
    }
}

/// The WCAG contrast ratio of two `#rrggbb` colors, for an editor to warn before `render` refuses.
#[wasm_bindgen]
pub fn contrast(dark: &str, light: &str) -> Result<f64, JsValue> {
    let pair = color(dark).and_then(|dark| Ok((dark, color(light)?)));
    pair.map(|(dark, light)| dark.contrast_ratio(light))
        .map_err(|refusal| JsValue::from_str(&refusal.message))
}

/// The lowest contrast `render` accepts.
#[wasm_bindgen(js_name = minContrast)]
#[must_use]
pub fn min_contrast() -> f64 {
    styled_qr::MIN_CONTRAST
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options() -> Options {
        defaults()
    }

    #[test]
    fn every_shape_and_finder_draws() {
        for shape in [Shape::Dots, Shape::Rounded, Shape::Connected, Shape::Square] {
            for finder in [Finder::Rounded, Finder::Square] {
                let drawn = draw(
                    "https://bipa.app",
                    &Options {
                        shape,
                        finder,
                        png: true,
                        ..options()
                    },
                    None,
                )
                .expect("draws");
                assert!(drawn.svg.starts_with("<svg"), "{shape:?} {finder:?}");
                assert!(drawn.png.is_some_and(|png| png.starts_with(b"\x89PNG")));
            }
        }
    }

    #[test]
    fn a_low_contrast_pair_is_refused() {
        let refusal = draw(
            "https://bipa.app",
            &Options {
                colors: Some(Colors {
                    dark: "#96c8c4".to_owned(),
                    light: "#ffffff".to_owned(),
                    finder: None,
                }),
                ..options()
            },
            None,
        )
        .expect_err("refused");
        assert_eq!(refusal.code, "contrast");
    }

    #[test]
    fn a_bad_color_or_logo_says_which() {
        let bad_color = Options {
            colors: Some(Colors {
                dark: "black".to_owned(),
                light: "#ffffff".to_owned(),
                finder: None,
            }),
            ..options()
        };
        assert_eq!(
            draw("x", &bad_color, None).expect_err("refused").code,
            "color"
        );
        assert_eq!(
            draw("x", &options(), Some(b"not a png"))
                .expect_err("refused")
                .code,
            "logoImage"
        );
    }

    #[test]
    fn a_logo_too_large_for_the_code_is_refused_when_shrinking_is_not_enough() {
        let mut png = Vec::new();
        image::RgbaImage::from_pixel(64, 64, image::Rgba([0, 0, 0, 255]))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("png");
        let options = Options {
            ec: Ec::L,
            logo: Some(LogoOptions {
                width: Some(0.6),
                padding: 2,
                plate: None,
                href: None,
            }),
            ..options()
        };
        assert_eq!(
            draw("https://bipa.app", &options, Some(&png))
                .expect_err("refused")
                .code,
            "logoUnreadable"
        );
    }

    #[test]
    fn a_linked_logo_leaves_the_svg_and_stays_in_the_png() {
        let mut png = Vec::new();
        image::RgbaImage::from_pixel(64, 64, image::Rgba([0, 0, 0, 255]))
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .expect("png");
        let plated = |href: Option<&str>| Options {
            logo: Some(LogoOptions {
                width: None,
                padding: 2,
                plate: Some(PlateOptions {
                    color: "#ffffff".to_owned(),
                    margin: 0.1,
                    roundness: 0.2,
                }),
                href: href.map(str::to_owned),
            }),
            png: true,
            ..options()
        };
        let linked = draw(
            "https://bipa.app",
            &plated(Some("/qr/logo.png?v=1&a=b")),
            Some(&png),
        )
        .expect("draws");
        let embedded = draw("https://bipa.app", &plated(None), Some(&png)).expect("draws");
        assert!(
            linked
                .svg
                .contains(r#"xlink:href="/qr/logo.png?v=1&amp;a=b""#)
        );
        assert!(!linked.svg.contains("base64"));
        assert!(linked.svg.contains(r##"<g fill="#ffffff"><path d="##));
        assert!(embedded.svg.contains("data:image/png;base64,"));
        assert_eq!(linked.png, embedded.png);
        let bare = draw("https://bipa.app", &plated(Some("/qr/logo.png")), None).expect("draws");
        assert!(!bare.svg.contains("<image"));
    }
}
