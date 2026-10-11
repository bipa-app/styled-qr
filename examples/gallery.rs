//! Renders the README gallery into `assets/`. Run it from the repository root:
//! `cargo run --example gallery`.

use std::error::Error;

use image::DynamicImage;
use styled_qr::{
    Color, Composer, EcLevel, Layout, Logo, ModuleShape, ModuleSize, Palette, Plate, QuietZone,
    Ratio, Roundness, Scene, Style, Symbol,
};

const DATA: &[u8] = b"https://github.com/bipa-app/styled-qr";

fn main() -> Result<(), Box<dyn Error>> {
    let symbol = Symbol::encode(DATA, EcLevel::H)?;
    let layout = Layout::new(ModuleSize::new(10)?, QuietZone::new(4)?);

    let scene = Composer::new(&symbol, layout).compose()?;
    save("dots", &scene)?;

    let scene = Composer::new(&symbol, layout)
        .style(Style::CONNECTED)
        .compose()?;
    save("connected", &scene)?;

    let scene = Composer::new(&symbol, layout)
        .style(Style::CLASSIC)
        .compose()?;
    save("classic", &scene)?;

    let logo = Logo::new(&image::open("assets/badge.png")?)?;
    let scene = Composer::new(&symbol, layout).logo(&logo).compose()?;
    save("logo", &scene)?;

    let logo = Logo::new(&image::open("assets/mark.png")?)?.with_plate(Plate {
        color: Color::rgb(124, 58, 237),
        margin: Ratio::new(0.2)?,
        roundness: Roundness::new(0.4)?,
    });
    let scene = Composer::new(&symbol, layout).logo(&logo).compose()?;
    save("plate", &scene)?;

    let style = Style {
        module: ModuleShape::Rounded {
            size: Ratio::new(0.9)?,
            roundness: Roundness::new(0.5)?,
        },
        palette: Palette::new(Color::rgb(30, 41, 59), Color::rgb(248, 250, 252))?
            .with_finder(Color::rgb(124, 58, 237))?,
        ..Style::DOTS
    };
    let scene = Composer::new(&symbol, layout).style(style).compose()?;
    std::fs::write("assets/colors.svg", scene.to_svg()?)?;
    save("colors", &scene)?;

    Ok(())
}

fn save(name: &str, scene: &Scene) -> Result<(), Box<dyn Error>> {
    let gray = DynamicImage::ImageRgba8(scene.to_image()).into_luma8();
    let width = usize::try_from(gray.width())?;
    let height = usize::try_from(gray.height())?;
    let decoded: Vec<Vec<u8>> = quircs::Quirc::default()
        .identify(width, height, &gray)
        .filter_map(Result::ok)
        .filter_map(|code| code.decode().ok())
        .map(|data| data.payload)
        .collect();
    if decoded != [DATA] {
        return Err(format!("{name} does not decode").into());
    }
    std::fs::write(format!("assets/{name}.png"), scene.to_png()?)?;
    Ok(())
}
