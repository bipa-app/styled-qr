use qrcode::types::QrError;

use crate::symbol::Role;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("failed to encode data: {0}")]
    Encode(QrError),
    #[error("version must be within 1..=40, got {0}")]
    Version(u8),
    #[error("module size must be within 1..={max} pixels, got {px}")]
    ModuleSize { px: u64, max: u32 },
    #[error("quiet zone must be within 1..={max} modules, got {modules}")]
    QuietZone { modules: u64, max: u8 },
    #[error("logo padding must be at most {max} modules, got {modules}")]
    Padding { modules: u64, max: u8 },
    #[error("ratio must be finite and within (0, 1], got {0}")]
    Ratio(f64),
    #[error("roundness must be finite and within [0, 1], got {0}")]
    Roundness(f64),
    #[error(
        "pattern shapes must nest: the stone inside the hole, the hole inside the outer square"
    )]
    PatternProportions,
    #[error("canvas of {px}px exceeds the {max}px limit")]
    CanvasTooLarge { px: u64, max: u32 },
    #[error("module size {px}px is below the {min}px minimum for this module shape")]
    ModuleTooSmall { px: u32, min: u32 },
    #[error("contrast ratio {ratio:.2} between dark and light colors is below {min:.2}")]
    InsufficientContrast { ratio: f64, min: f64 },
    #[error("logo makes {0:?} modules unreadable, and error correction cannot restore them")]
    LogoHidesFunctionPattern(Role),
    #[error("logo hides {used} codewords of a block that corrects at most {budget}")]
    LogoExceedsErrorBudget { used: u16, budget: u16 },
    #[error("logo image has no pixels")]
    EmptyLogo,
    #[error("logo of {width}x{height}px does not fit within 2..={max}px")]
    LogoDoesNotFit { width: u64, height: u64, max: u32 },
    #[error("failed to encode image: {0}")]
    Image(#[from] image::ImageError),
}
