# styled-qr

Styled QR codes with exact anti-aliasing, logos, and PNG and SVG output.

Each pixel gets the exact area of the shapes that cover it, so dots, finder
patterns and logos stay symmetric and centered at every module size. The PNG
and the SVG come from the same scene.

## Usage

```toml
[dependencies]
styled-qr = { git = "https://github.com/bipa-app/styled-qr" }
```

```rust
use styled_qr::{Composer, EcLevel, Layout, Logo, ModuleSize, QuietZone, Style, Symbol};

fn main() -> Result<(), styled_qr::Error> {
    let symbol = Symbol::encode(b"https://example.com", EcLevel::H)?;
    let layout = Layout::new(ModuleSize::new(12)?, QuietZone::new(4)?);
    let logo = Logo::new(&image::RgbaImage::from_pixel(64, 64, image::Rgba([0, 0, 0, 255])))?;
    let scene = Composer::new(&symbol, layout)
        .style(Style::CONNECTED)
        .logo(&logo)
        .compose()?;
    let png = scene.to_png()?;
    let svg = scene.to_svg()?;
    assert_eq!(scene.size(), (u32::from(symbol.width()) + 8) * 12);
    assert!(png.starts_with(b"\x89PNG") && svg.starts_with("<svg"));
    Ok(())
}
```

`Layout::fit` picks the largest module size that fits a pixel budget.

## Styles

| Preset | Modules | Finder patterns | Alignment patterns |
| --- | --- | --- | --- |
| `Style::DOTS` (default) | dots | square frame, rounded hole and stone | rounded |
| `Style::CONNECTED` | tiles that join their dark neighbors | as `DOTS` | as `DOTS` |
| `Style::CLASSIC` | squares | squares | plain modules |

Other styles combine `ModuleShape`, `PatternShape`, `AlignmentStyle`, `Palette`
and `Blending`. `Palette::new` refuses colors with a contrast ratio below 3:1.
`Blending::Linear` mixes edge pixels in linear light.

Rounded outer finder corners move the corner estimates of quirc-family
decoders, so the presets keep the outer frame square.

## Logos

The composer clears the modules behind the logo. Then it counts the codewords
that the logo makes read wrong in each error correction block. It refuses a
logo that:

- uses more than its error budget, by default 75% of the correction capacity
  of the worst block,
- hides a finder, separator or timing module,
- damages the format or version information more than BCH can correct.

With `OverBudget::Shrink`, the default, the composer tries the logo at 90% down
to 50% of its size before it returns the error. `Clearance` selects the area to
clear: the convex hull of the logo (default), its opaque pixels, or its
bounding box. `Plate` draws a rounded backing behind the logo.

## Tests

The tests decode the output with [quircs](https://crates.io/crates/quircs),
compare each symbol with the [qrcode](https://crates.io/crates/qrcode) crate,
and compare the coverage of each shape with supersampling.

## Credits

styled-qr started as the QR code renderer of the [Bipa](https://bipa.app)
backend. The commit history keeps each author of that work.

## License

MIT. See the `LICENSE` file.
