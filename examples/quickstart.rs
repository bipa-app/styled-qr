use styled_qr::{Composer, EcLevel, Layout, ModuleSize, QuietZone, Symbol};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let symbol = Symbol::encode(b"https://example.com", EcLevel::H)?;
    let layout = Layout::new(ModuleSize::new(10)?, QuietZone::new(4)?);
    let scene = Composer::new(&symbol, layout).compose()?;
    std::fs::write("qr.png", scene.to_png()?)?;
    std::fs::write("qr.svg", scene.to_svg()?)?;
    Ok(())
}
