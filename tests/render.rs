use image::{DynamicImage, GrayImage, Rgba, RgbaImage};
use qrcoder::{
    AlignmentStyle, Builder, Clearance, Color, Composer, EcLevel, Error, Layout, Logo, LogoSize,
    ModuleSize, OverBudget, Padding, Palette, Pos, QuietZone, Ratio, Role, Scene, Shape, Style,
    Symbol, Version,
};

#[derive(Clone, Copy)]
enum SampleLogo {
    Coin,
    Tall,
    Sparse,
}

fn load(kind: SampleLogo) -> DynamicImage {
    let (w, h) = match kind {
        SampleLogo::Coin => (400, 400),
        SampleLogo::Tall => (392, 398),
        SampleLogo::Sparse => (250, 250),
    };
    let image = RgbaImage::from_fn(w, h, |px, py| {
        let dx = (f64::from(px) + 0.5) / f64::from(w) - 0.5;
        let dy = (f64::from(py) + 0.5) / f64::from(h) - 0.5;
        let radius = dx.hypot(dy);
        match kind {
            SampleLogo::Coin if (dx - 0.3).hypot(dy - 0.3) < 0.15 => Rgba([40, 120, 250, 255]),
            SampleLogo::Coin | SampleLogo::Tall if radius < 0.12 => Rgba([255, 255, 255, 255]),
            SampleLogo::Coin | SampleLogo::Tall if radius < 0.48 => Rgba([0, 0, 0, 255]),
            SampleLogo::Sparse if (radius - 0.3).abs() < 0.06 => Rgba([200, 40, 120, 255]),
            _ => Rgba([255, 0, 0, 0]),
        }
    });
    DynamicImage::ImageRgba8(image)
}

fn decode(image: &RgbaImage) -> Vec<String> {
    let gray: GrayImage = DynamicImage::ImageRgba8(image.clone()).into_luma8();
    let width = usize::try_from(gray.width()).expect("width fits in usize");
    let height = usize::try_from(gray.height()).expect("height fits in usize");
    let mut decoder = quircs::Quirc::default();
    decoder
        .identify(width, height, &gray)
        .filter_map(Result::ok)
        .filter_map(|code| code.decode().ok())
        .map(|data| String::from_utf8_lossy(&data.payload).into_owned())
        .collect()
}

fn pixel(value: f64) -> u32 {
    format!("{:.0}", value.floor())
        .parse()
        .expect("pixel coordinate")
}

fn payload(len: usize) -> String {
    "bitcoin:bc1qar0srrr7xfkvy5l643lydnw9re59gtzzwf5mdq?amount=0.001"
        .chars()
        .cycle()
        .take(len)
        .collect()
}

fn layout(px: u32, quiet: u32) -> Layout {
    Layout::new(
        ModuleSize::new(px).expect("module size"),
        QuietZone::new(quiet).expect("quiet zone"),
    )
}

fn compose(symbol: &Symbol, layout: Layout, style: Style, logo: Option<&Logo>) -> Scene {
    let composer = Composer::new(symbol, layout).style(style);
    logo.map_or(composer, |l| composer.logo(l))
        .compose()
        .expect("compose")
}

#[test]
fn production_configurations_decode() {
    let app_logos = [
        load(SampleLogo::Coin),
        load(SampleLogo::Tall),
        load(SampleLogo::Sparse),
    ];
    for len in [1usize, 20, 42, 120, 300] {
        let data = payload(len);
        for logo in &app_logos {
            let image = Builder::new(data.as_bytes(), logo)
                .with_module_width(22)
                .build()
                .expect("app build")
                .into_rgba8();
            assert_eq!(decode(&image), vec![data.clone()], "app len {len}");
        }
        let site = Builder::new(data.as_bytes(), &app_logos[2])
            .with_quiet_zone(2)
            .with_logo_quiet_zone(2)
            .build()
            .expect("site build")
            .into_rgba8();
        assert_eq!(decode(&site), vec![data.clone()], "site len {len}");
    }
}

#[test]
fn builder_defaults_match_the_legacy_geometry() {
    let data = payload(42);
    let logo = load(SampleLogo::Coin);
    let image = Builder::new(data.as_bytes(), &logo).build().expect("build");
    let symbol = Symbol::encode(data.as_bytes(), EcLevel::H).expect("encode");
    assert_eq!(image.width(), (u32::from(symbol.width()) + 8) * 11);
    assert_eq!(image.width(), image.height());
}

#[test]
fn builder_rejects_invalid_configuration_instead_of_panicking() {
    let logo = load(SampleLogo::Coin);
    let data = payload(30);
    let build = |m: usize, q: usize, l: usize| {
        Builder::new(data.as_bytes(), &logo)
            .with_module_width(m)
            .with_quiet_zone(q)
            .with_logo_quiet_zone(l)
            .build()
    };
    assert!(matches!(build(0, 4, 1), Err(Error::ModuleSize { .. })));
    assert!(matches!(
        build(2, 4, 1),
        Err(Error::ModuleTooSmall { px: 2, min: 3 })
    ));
    assert!(matches!(build(1000, 4, 1), Err(Error::ModuleSize { .. })));
    assert!(matches!(build(22, 0, 1), Err(Error::QuietZone { .. })));
    let huge = usize::try_from(i32::MAX).expect("i32 fits in usize");
    assert!(matches!(build(22, huge, 1), Err(Error::QuietZone { .. })));
    assert!(matches!(build(22, 4, 100), Err(Error::Padding { .. })));
    assert!(matches!(
        build(256, 64, 1),
        Err(Error::CanvasTooLarge { .. })
    ));
    assert!(matches!(
        Builder::new(&[b'x'; 4000], &logo).build(),
        Err(Error::Encode(_))
    ));
    let empty = RgbaImage::new(0, 0);
    assert!(matches!(
        Builder::new(b"x", &empty).build(),
        Err(Error::EmptyLogo)
    ));
}

#[test]
fn presets_decode_without_logo() {
    for style in [Style::BRAND, Style::CLASSIC] {
        for len in [5usize, 80, 400] {
            let data = payload(len);
            let symbol = Symbol::encode(data.as_bytes(), EcLevel::M).expect("encode");
            let image = compose(&symbol, layout(8, 4), style, None).to_image();
            assert_eq!(decode(&image), vec![data], "{style:?}");
        }
    }
}

#[test]
fn every_visible_module_reads_back_its_color() {
    let data = payload(150);
    let symbol = Symbol::encode(data.as_bytes(), EcLevel::H).expect("encode");
    let logo = Logo::new(&load(SampleLogo::Coin)).expect("logo");
    let layout = layout(12, 4);
    let scene = compose(&symbol, layout, Style::BRAND, Some(&logo));
    let image = scene.to_image();
    let logo_rect = scene.logo_rect().expect("logo rect");
    let mut checked = 0;
    for (pos, module) in symbol.modules() {
        let center = layout.cell(pos).center();
        let near_logo = logo_rect.distance_to(center) <= 12.0 * 3.0;
        if near_logo || module.role == Role::Finder {
            continue;
        }
        let Rgba([r, ..]) = *image.get_pixel(pixel(center.x), pixel(center.y));
        assert_eq!(r < 128, module.dark, "{pos:?} {module:?}");
        checked += 1;
    }
    assert!(checked > 1000);
}

#[test]
fn quiet_zone_is_pure_background() {
    let symbol = Symbol::encode(b"quiet", EcLevel::H).expect("encode");
    let palette = Palette::new(Color::rgb(20, 30, 90), Color::rgb(250, 245, 230)).expect("palette");
    let style = Style {
        palette,
        ..Style::BRAND
    };
    let image = compose(&symbol, layout(10, 3), style, None).to_image();
    let edge = 30;
    for (x, y, pixel) in image.enumerate_pixels() {
        let inside =
            (edge..image.width() - edge).contains(&x) && (edge..image.height() - edge).contains(&y);
        if !inside {
            assert_eq!(*pixel, Rgba([250, 245, 230, 255]), "({x},{y})");
        }
    }
    assert!(image.pixels().any(|p| *p == Rgba([20, 30, 90, 255])));
}

#[test]
fn transparent_background_keeps_alpha() {
    let symbol = Symbol::encode(b"alpha", EcLevel::H).expect("encode");
    let style = Style {
        palette: Palette::new(Color::BLACK, Color::TRANSPARENT).expect("palette"),
        ..Style::BRAND
    };
    let image = compose(&symbol, layout(10, 4), style, None).to_image();
    assert_eq!(image.get_pixel(0, 0)[3], 0);
    let finder_ring = image.get_pixel(41, 41);
    assert_eq!(*finder_ring, Rgba([0, 0, 0, 255]));
    assert!(image.pixels().any(|p| p[3] > 0 && p[3] < 255));
}

#[test]
fn finder_patterns_are_pixel_symmetric() {
    for px in [7u32, 10, 13, 22] {
        let symbol = Symbol::encode(b"symmetry", EcLevel::H).expect("encode");
        let image = compose(&symbol, layout(px, 4), Style::BRAND, None).to_image();
        let side = 7 * px;
        let origin = 4 * px;
        for y in 0..side {
            for x in 0..side {
                let p = image.get_pixel(origin + x, origin + y);
                assert_eq!(p, image.get_pixel(origin + side - 1 - x, origin + y));
                assert_eq!(p, image.get_pixel(origin + x, origin + side - 1 - y));
                assert_eq!(p, image.get_pixel(origin + y, origin + x));
            }
        }
    }
}

#[test]
fn dots_carry_exact_ink() {
    let symbol = Symbol::encode(b"ink", EcLevel::H).expect("encode");
    let px = 21u32;
    let layout = layout(px, 4);
    let image = compose(&symbol, layout, Style::BRAND, None).to_image();
    let radius = f64::from(px) * 0.85 / 2.0;
    let expected = std::f64::consts::PI * radius * radius;
    let dots = symbol
        .modules()
        .filter(|(_, m)| m.dark && m.role == Role::Data)
        .take(20);
    for (pos, _) in dots {
        let cell = layout.cell(pos);
        let (x0, y0) = (pixel(cell.min().x), pixel(cell.min().y));
        let ink: f64 = (0..px)
            .flat_map(|y| (0..px).map(move |x| (x, y)))
            .map(|(x, y)| 1.0 - f64::from(image.get_pixel(x0 + x, y0 + y)[0]) / 255.0)
            .sum();
        assert!((ink - expected).abs() < 0.5, "{pos:?}: {ink} vs {expected}");
    }
}

#[test]
fn rendering_is_deterministic() {
    let logo = Logo::new(&load(SampleLogo::Coin)).expect("logo");
    let symbol = Symbol::encode(payload(90).as_bytes(), EcLevel::H).expect("encode");
    let a = compose(&symbol, layout(9, 4), Style::BRAND, Some(&logo));
    let b = compose(&symbol, layout(9, 4), Style::BRAND, Some(&logo));
    assert_eq!(a.to_image(), b.to_image());
    assert_eq!(a.to_svg().expect("svg"), b.to_svg().expect("svg"));
}

#[test]
fn symmetric_logo_is_centered_with_symmetric_clearance() {
    let square = RgbaImage::from_pixel(90, 90, Rgba([10, 10, 10, 255]));
    let logo = Logo::new(&square)
        .expect("logo")
        .with_clearance(Clearance::Bounds)
        .with_size(LogoSize::Width(Ratio::new(0.2).expect("ratio")))
        .with_padding(Padding::new(1).expect("padding"));
    for len in [30usize, 31, 200] {
        let symbol = Symbol::encode(payload(len).as_bytes(), EcLevel::H).expect("encode");
        for px in [10u32, 11] {
            let scene = compose(&symbol, layout(px, 4), Style::CLASSIC, Some(&logo));
            let rect = scene.logo_rect().expect("logo");
            let size = f64::from(scene.size());
            assert!((rect.center().x - size / 2.0).abs() < 1e-9);
            assert!((rect.center().y - size / 2.0).abs() < 1e-9);
            let image = scene.to_image();
            let grid = layout(px, 4);
            let w = symbol.width();
            let hidden = |p: Pos| rect.distance_to(grid.cell(p).center()) <= f64::from(px);
            for (pos, module) in symbol.modules() {
                let mirror = Pos::new(w - 1 - pos.x, w - 1 - pos.y);
                assert_eq!(hidden(pos), hidden(mirror), "{pos:?}");
                let center = grid.cell(pos).center();
                let outside_logo = rect.distance_to(center) > 0.0;
                if module.dark && module.role == Role::Data && hidden(pos) && outside_logo {
                    assert_eq!(
                        image.get_pixel(pixel(center.x), pixel(center.y))[0],
                        255,
                        "{pos:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn footprint_clearance_hides_fewer_modules_than_bounds() {
    let symbol = Symbol::encode(payload(120).as_bytes(), EcLevel::H).expect("encode");
    let logo = Logo::new(&load(SampleLogo::Sparse)).expect("logo");
    let count = |clearance| {
        let logo = logo.clone().with_clearance(clearance);
        let scene = compose(&symbol, layout(10, 4), Style::BRAND, Some(&logo));
        scene.layers().iter().map(|l| l.shapes.len()).sum::<usize>()
    };
    assert!(count(Clearance::Footprint) > count(Clearance::Bounds));
}

#[test]
fn oversized_logo_is_rejected_or_shrunk() {
    let symbol = Symbol::encode(b"tiny", EcLevel::H).expect("encode");
    let block = RgbaImage::from_pixel(64, 64, Rgba([0, 0, 0, 255]));
    let big = Logo::new(&block)
        .expect("logo")
        .with_size(LogoSize::Width(Ratio::new(0.45).expect("ratio")))
        .with_over_budget(OverBudget::Reject);
    let rejected = Composer::new(&symbol, layout(10, 4)).logo(&big).compose();
    assert!(matches!(
        rejected,
        Err(Error::LogoExceedsErrorBudget { .. })
    ));
    let shrinking = big.clone().with_over_budget(OverBudget::Shrink);
    let scene = Composer::new(&symbol, layout(10, 4))
        .logo(&shrinking)
        .compose()
        .expect("shrunk");
    let side = scene.logo_rect().expect("logo").width();
    assert!(side < 0.45 * 21.0 * 10.0);
    assert!(scene.error_budget_usage().fraction() <= Logo::DEFAULT_ERROR_BUDGET.get());
    assert_eq!(decode(&scene.to_image()), vec!["tiny".to_owned()]);
}

#[test]
fn alignment_marks_under_the_logo_are_dropped() {
    let version = Version::new(10).expect("version");
    let symbol = Symbol::encode_with(payload(40).as_bytes(), EcLevel::H, version).expect("encode");
    assert_eq!(symbol.version(), version);
    let logo = Logo::new(&load(SampleLogo::Coin)).expect("logo");
    let layout = layout(10, 4);
    let with_logo = compose(&symbol, layout, Style::BRAND, Some(&logo));
    let without = compose(&symbol, layout, Style::BRAND, None);
    let marks = |scene: &Scene| scene.layers().get(2).map_or(0, |l| l.shapes.len());
    assert_eq!(marks(&without), 2 * symbol.alignment_centers().len());
    assert_eq!(marks(&with_logo), marks(&without) - 2);
    let style = Style {
        alignment: AlignmentStyle::Modules,
        ..Style::BRAND
    };
    assert_eq!(compose(&symbol, layout, style, None).layers().len(), 2);
}

#[test]
fn svg_mirrors_the_scene() {
    let symbol = Symbol::encode(payload(60).as_bytes(), EcLevel::H).expect("encode");
    let logo = Logo::new(&load(SampleLogo::Coin)).expect("logo");
    let scene = compose(&symbol, layout(10, 4), Style::BRAND, Some(&logo));
    let svg = scene.to_svg().expect("svg");
    let discs = scene
        .layers()
        .iter()
        .flat_map(|l| &l.shapes)
        .filter(|s| matches!(s, Shape::Disc(_)))
        .count();
    let rings = scene
        .layers()
        .iter()
        .flat_map(|l| &l.shapes)
        .filter(|s| matches!(s, Shape::Ring(_)))
        .count();
    assert_eq!(svg.matches("<circle").count(), discs);
    assert_eq!(svg.matches("fill-rule=\"evenodd\"").count(), rings);
    assert_eq!(svg.matches("<image").count(), 1);
    assert!(svg.contains("data:image/png;base64,"));
    assert!(svg.starts_with(&format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{0}" height="{0}""#,
        scene.size()
    )));
    assert!(svg.ends_with("</svg>"));
    let classic = compose(&symbol, layout(10, 4), Style::CLASSIC, None)
        .to_svg()
        .expect("svg");
    assert_eq!(classic.matches("<circle").count(), 0);
    assert!(classic.matches("<rect").count() > 100);
}

#[test]
fn minimum_version_renders_a_larger_symbol() {
    let small = Symbol::encode(b"hi", EcLevel::H).expect("encode");
    let large =
        Symbol::encode_with(b"hi", EcLevel::H, Version::new(4).expect("v")).expect("encode");
    assert_eq!(large.width(), 33);
    let image = compose(&large, layout(6, 4), Style::BRAND, None).to_image();
    assert_eq!(decode(&image), vec!["hi".to_owned()]);
    assert!(large.width() > small.width());
}

#[test]
fn layout_fits_the_requested_size() {
    let symbol = Symbol::encode(payload(42).as_bytes(), EcLevel::H).expect("encode");
    let quiet = QuietZone::new(3).expect("quiet");
    let modules = u32::from(symbol.width()) + 6;
    let fitted = Layout::fit(&symbol, 512, quiet).expect("fit");
    assert_eq!(fitted.module_size.px(), 512 / modules);
    assert!(fitted.canvas_px(&symbol).expect("canvas") <= 512);
    let next = Layout::new(
        ModuleSize::new(fitted.module_size.px() + 1).expect("m"),
        quiet,
    );
    assert!(next.canvas_px(&symbol).expect("canvas") > 512);
    assert!(matches!(
        Layout::fit(&symbol, modules - 1, quiet),
        Err(Error::ModuleSize { .. })
    ));
    assert_eq!(
        Layout::fit(&symbol, u32::MAX, quiet)
            .expect("fit")
            .module_size
            .px(),
        qrcoder::MAX_MODULE_PX
    );
}

#[test]
fn png_export_round_trips() {
    let symbol = Symbol::encode(b"png", EcLevel::H).expect("encode");
    let scene = compose(&symbol, layout(8, 4), Style::BRAND, None);
    let png = scene.to_png().expect("png");
    let decoded = image::load_from_memory(&png).expect("decode").into_rgba8();
    assert_eq!(decoded, scene.to_image());
}

#[test]
fn custom_layers_render_on_top() {
    let symbol = Symbol::encode(b"layers", EcLevel::H).expect("encode");
    let mut scene = compose(&symbol, layout(10, 4), Style::BRAND, None);
    let before = scene.layers().len();
    let badge = qrcoder::Rect::from_origin(qrcoder::Point::new(2.0, 2.0), 6.0, 6.0);
    scene.push_layer(qrcoder::Layer {
        color: Color::rgb(200, 0, 0),
        shapes: vec![Shape::Rect(badge)],
    });
    assert_eq!(scene.layers().len(), before + 1);
    let image = scene.to_image();
    assert_eq!(*image.get_pixel(4, 4), Rgba([200, 0, 0, 255]));
    assert_eq!(*image.get_pixel(9, 9), Rgba([255, 255, 255, 255]));
}

#[test]
fn smallest_supported_module_size_renders() {
    let symbol = Symbol::encode(b"min", EcLevel::H).expect("encode");
    let min = Style::BRAND.module.min_module_px();
    let scene = Composer::new(&symbol, layout(min, 4))
        .compose()
        .expect("min size");
    assert_eq!(decode(&scene.to_image()), vec!["min".to_owned()]);
    assert!(matches!(
        Composer::new(&symbol, layout(min - 1, 4)).compose(),
        Err(Error::ModuleTooSmall { .. })
    ));
    let classic = Composer::new(&symbol, layout(1, 4))
        .style(Style::CLASSIC)
        .compose();
    assert!(classic.is_ok());
}

#[test]
fn budget_counts_only_modules_that_read_wrong() {
    let symbol = Symbol::encode(payload(90).as_bytes(), EcLevel::H).expect("encode");
    let px = 10u32;
    let grid = layout(px, 4);
    let invisible = RgbaImage::from_pixel(40, 40, Rgba([0, 0, 0, 0]));
    for padding in [0u32, 1, 2] {
        let logo = Logo::new(&invisible)
            .expect("logo")
            .with_clearance(Clearance::Bounds)
            .with_padding(Padding::new(padding).expect("padding"));
        let scene = compose(&symbol, grid, Style::BRAND, Some(&logo));
        let rect = scene.logo_rect().expect("logo");
        let extent = f64::from(px) * 0.85 / 2.0;
        let clearance = (f64::from(padding) * f64::from(px)).max(extent);
        let hidden_dark: Vec<Pos> = symbol
            .modules()
            .filter(|(pos, m)| m.dark && rect.distance_to(grid.cell(*pos).center()) <= clearance)
            .map(|(pos, _)| pos)
            .collect();
        let dots = symbol
            .modules()
            .filter(|(pos, m)| {
                m.dark
                    && matches!(
                        m.role,
                        Role::Data | Role::Timing | Role::FormatInfo | Role::VersionInfo
                    )
                    && rect.distance_to(grid.cell(*pos).center()) > clearance
            })
            .count();
        assert_eq!(scene.layers().first().map(|l| l.shapes.len()), Some(dots));
        let expected = qrcoder::BudgetUsage::of(&symbol, hidden_dark);
        assert_eq!(scene.error_budget_usage(), expected, "padding {padding}");
        assert!(expected.worst_block() > 0);
    }
}

#[test]
fn legacy_logo_size_follows_the_area_formula() {
    let symbol = Symbol::encode(payload(42).as_bytes(), EcLevel::H).expect("encode");
    let square = RgbaImage::from_pixel(300, 300, Rgba([0, 0, 0, 255]));
    let logo = Logo::new(&square)
        .expect("logo")
        .with_over_budget(OverBudget::Reject);
    for px in [10u32, 11, 22] {
        let scene = compose(&symbol, layout(px, 4), Style::BRAND, Some(&logo));
        let m = f64::from(px);
        let symbol_px = f64::from(symbol.width()) * m;
        let finder_px = 7.0 * m;
        let expected = ((symbol_px * symbol_px - 3.0 * finder_px * finder_px) * 0.05).sqrt();
        let side = scene.logo_rect().expect("logo").width();
        assert!((side - expected).abs() <= 1.5, "{px}: {side} vs {expected}");
    }
}

#[test]
fn svg_background_follows_alpha() {
    let symbol = Symbol::encode(b"bg", EcLevel::H).expect("encode");
    let svg_with = |light: Color| {
        let style = Style {
            palette: Palette::new(Color::BLACK, light).expect("palette"),
            ..Style::BRAND
        };
        compose(&symbol, layout(10, 4), style, None)
            .to_svg()
            .expect("svg")
    };
    let size = (u32::from(symbol.width()) + 8) * 10;
    let background = format!(r#"<rect width="{size}" height="{size}""#);
    assert!(svg_with(Color::WHITE).contains(&format!(r##"{background} fill="#ffffff"/>"##)));
    assert!(!svg_with(Color::TRANSPARENT).contains(&background));
    assert!(svg_with(Color::rgba(255, 255, 255, 1)).contains(&format!(
        r##"{background} fill="#ffffff" fill-opacity="0.0039"/>"##
    )));
}

#[test]
fn logo_pixels_land_exactly_on_the_canvas() {
    let symbol = Symbol::encode(payload(42).as_bytes(), EcLevel::H).expect("encode");
    let marker = RgbaImage::from_fn(20, 20, |x, y| {
        if x < 10 && y < 10 {
            Rgba([255, 0, 0, 255])
        } else {
            Rgba([0, 0, 255, 255])
        }
    });
    let logo = Logo::new(&marker)
        .expect("logo")
        .with_size(LogoSize::Width(Ratio::new(0.15).expect("ratio")));
    let scene = compose(&symbol, layout(10, 4), Style::BRAND, Some(&logo));
    let rect = scene.logo_rect().expect("logo");
    let image = scene.to_image();
    let (x0, y0) = (pixel(rect.min().x), pixel(rect.min().y));
    let (x1, y1) = (pixel(rect.max().x), pixel(rect.max().y));
    assert_eq!(*image.get_pixel(x0 + 2, y0 + 2), Rgba([255, 0, 0, 255]));
    assert_eq!(*image.get_pixel(x1 - 3, y1 - 3), Rgba([0, 0, 255, 255]));
    assert_eq!(*image.get_pixel(x0 + 2, y1 - 3), Rgba([0, 0, 255, 255]));
    assert_ne!(*image.get_pixel(x0 - 1, y0 + 2), Rgba([255, 0, 0, 255]));
    assert_eq!(*image.get_pixel(x0 - 1, y0 + 2), Rgba([255, 255, 255, 255]));
}

#[test]
fn same_color_layers_meet_without_seams() {
    let symbol = Symbol::encode(b"seam", EcLevel::H).expect("encode");
    let mut scene = compose(&symbol, layout(10, 4), Style::BRAND, None);
    let left = qrcoder::Rect::from_corners(
        qrcoder::Point::new(2.0, 2.0),
        qrcoder::Point::new(7.5, 12.0),
    );
    let right = qrcoder::Rect::from_corners(
        qrcoder::Point::new(7.5, 2.0),
        qrcoder::Point::new(13.0, 12.0),
    );
    for rect in [left, right] {
        scene.push_layer(qrcoder::Layer {
            color: Color::rgb(0, 0, 0),
            shapes: vec![Shape::Rect(rect)],
        });
    }
    let red = qrcoder::Rect::from_corners(
        qrcoder::Point::new(2.0, 14.0),
        qrcoder::Point::new(7.5, 18.0),
    );
    let blue = qrcoder::Rect::from_corners(
        qrcoder::Point::new(7.5, 14.0),
        qrcoder::Point::new(13.0, 18.0),
    );
    scene.push_layer(qrcoder::Layer {
        color: Color::rgb(255, 0, 0),
        shapes: vec![Shape::Rect(red)],
    });
    scene.push_layer(qrcoder::Layer {
        color: Color::rgb(0, 0, 255),
        shapes: vec![Shape::Rect(blue)],
    });
    let image = scene.to_image();
    assert_eq!(*image.get_pixel(7, 5), Rgba([0, 0, 0, 255]));
    assert_eq!(*image.get_pixel(6, 5), Rgba([0, 0, 0, 255]));
    assert_eq!(*image.get_pixel(12, 5), Rgba([0, 0, 0, 255]));
    assert_eq!(*image.get_pixel(13, 5), Rgba([255, 255, 255, 255]));
    let Rgba([r, g, b, _]) = *image.get_pixel(7, 15);
    assert_eq!((r, g, b), (128, 64, 191));
}
