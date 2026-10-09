use std::num::NonZeroU32;

use crate::error::Error;

pub const MAX_MODULE_PX: u32 = 256;
pub const MAX_CANVAS_PX: u32 = 16_384;
pub const MAX_QUIET_ZONE: u8 = 64;
pub const MAX_PADDING: u8 = 16;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ModuleSize(NonZeroU32);

impl ModuleSize {
    pub fn new(px: u32) -> Result<Self, Error> {
        NonZeroU32::new(px)
            .filter(|px| px.get() <= MAX_MODULE_PX)
            .map(Self)
            .ok_or(Error::ModuleSize {
                px: u64::from(px),
                max: MAX_MODULE_PX,
            })
    }

    #[must_use]
    pub const fn px(self) -> u32 {
        self.0.get()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct QuietZone(u8);

impl QuietZone {
    pub const STANDARD: Self = Self(4);

    pub fn new(modules: u32) -> Result<Self, Error> {
        u8::try_from(modules)
            .ok()
            .filter(|m| (1..=MAX_QUIET_ZONE).contains(m))
            .map(Self)
            .ok_or(Error::QuietZone {
                modules: u64::from(modules),
                max: MAX_QUIET_ZONE,
            })
    }

    #[must_use]
    pub const fn modules(self) -> u8 {
        self.0
    }
}

impl Default for QuietZone {
    fn default() -> Self {
        Self::STANDARD
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Padding(u8);

impl Padding {
    pub fn new(modules: u32) -> Result<Self, Error> {
        u8::try_from(modules)
            .ok()
            .filter(|m| *m <= MAX_PADDING)
            .map(Self)
            .ok_or(Error::Padding {
                modules: u64::from(modules),
                max: MAX_PADDING,
            })
    }

    #[must_use]
    pub const fn modules(self) -> u8 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Ratio(f64);

impl Ratio {
    pub const ONE: Self = Self(1.0);

    pub const fn new(value: f64) -> Result<Self, Error> {
        if Self::is_valid(value) {
            Ok(Self(value))
        } else {
            Err(Error::Ratio(value))
        }
    }

    pub(crate) const fn or_one(value: f64) -> Self {
        if Self::is_valid(value) {
            Self(value)
        } else {
            Self::ONE
        }
    }

    const fn is_valid(value: f64) -> bool {
        value.is_finite() && value > 0.0 && value <= 1.0
    }

    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, PartialOrd)]
pub struct Roundness(f64);

impl Roundness {
    pub const SQUARE: Self = Self(0.0);
    pub const CIRCLE: Self = Self(1.0);

    pub const fn new(value: f64) -> Result<Self, Error> {
        if value.is_finite() && value >= 0.0 && value <= 1.0 {
            Ok(Self(value))
        } else {
            Err(Error::Roundness(value))
        }
    }

    pub(crate) const fn or_square(value: f64) -> Self {
        match value {
            v if v.is_finite() && v >= 0.0 && v <= 1.0 => Self(v),
            _ => Self::SQUARE,
        }
    }

    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }

    #[must_use]
    pub fn radius_for(self, side: f64) -> f64 {
        side * self.0 / 2.0
    }
}

pub(crate) fn floor_clamped(value: f64, max: u32) -> u32 {
    if value.is_nan() || value <= 0.0 {
        return 0;
    }
    let (mut lo, mut hi) = (0, max);
    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        if f64::from(mid) <= value {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    lo
}

pub(crate) fn round_clamped(value: f64, max: u32) -> u32 {
    floor_clamped(value + 0.5, max)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_size_rejects_zero_and_oversized() {
        assert!(ModuleSize::new(0).is_err());
        assert!(ModuleSize::new(MAX_MODULE_PX + 1).is_err());
        assert_eq!(ModuleSize::new(1).map(ModuleSize::px).ok(), Some(1));
        assert_eq!(
            ModuleSize::new(MAX_MODULE_PX).map(ModuleSize::px).ok(),
            Some(MAX_MODULE_PX)
        );
    }

    #[test]
    fn quiet_zone_bounds() {
        assert!(QuietZone::new(0).is_err());
        assert!(QuietZone::new(u32::from(MAX_QUIET_ZONE) + 1).is_err());
        assert!(QuietZone::new(300).is_err());
        assert_eq!(QuietZone::new(2).map(QuietZone::modules).ok(), Some(2));
        assert_eq!(QuietZone::default().modules(), 4);
    }

    #[test]
    fn padding_bounds() {
        assert_eq!(Padding::new(0).map(Padding::modules).ok(), Some(0));
        assert_eq!(
            Padding::new(u32::from(MAX_PADDING))
                .map(Padding::modules)
                .ok(),
            Some(MAX_PADDING)
        );
        assert!(Padding::new(u32::from(MAX_PADDING) + 1).is_err());
        assert!(Padding::new(1000).is_err());
    }

    #[test]
    fn ratio_rejects_out_of_range_and_non_finite() {
        for bad in [0.0, -0.1, 1.000_001, f64::NAN, f64::INFINITY] {
            assert!(Ratio::new(bad).is_err(), "{bad}");
        }
        assert_eq!(Ratio::new(1.0).map(Ratio::get).ok(), Some(1.0));
        assert_eq!(Ratio::new(0.25).map(Ratio::get).ok(), Some(0.25));
        assert!((Ratio::or_one(2.0).get() - 1.0).abs() < 1e-12);
        assert!((Ratio::or_one(0.5).get() - 0.5).abs() < 1e-12);
    }

    #[test]
    fn roundness_bounds() {
        for bad in [-0.01, 1.01, f64::NAN, f64::NEG_INFINITY] {
            assert!(Roundness::new(bad).is_err(), "{bad}");
            assert_eq!(Roundness::or_square(bad), Roundness::SQUARE);
        }
        assert_eq!(Roundness::new(0.0).ok(), Some(Roundness::SQUARE));
        assert_eq!(Roundness::new(1.0).ok(), Some(Roundness::CIRCLE));
        assert!((Roundness::or_square(0.5).get() - 0.5).abs() < 1e-12);
        assert!((Roundness::CIRCLE.radius_for(10.0) - 5.0).abs() < 1e-12);
        assert!((Roundness::SQUARE.radius_for(10.0) - 0.0).abs() < 1e-12);
    }

    #[test]
    fn pixel_rounding_is_exact_and_clamped() {
        assert_eq!(floor_clamped(f64::NAN, 10), 0);
        assert_eq!(floor_clamped(-3.2, 10), 0);
        assert_eq!(floor_clamped(0.0, 10), 0);
        assert_eq!(floor_clamped(3.999, 10), 3);
        assert_eq!(floor_clamped(4.0, 10), 4);
        assert_eq!(floor_clamped(42.0, 10), 10);
        assert_eq!(round_clamped(2.49, 10), 2);
        assert_eq!(round_clamped(2.5, 10), 3);
        for v in 0..2000 {
            let x = f64::from(v) / 7.0;
            assert!((f64::from(floor_clamped(x, 1000)) - x.floor().min(1000.0)).abs() < 1e-12);
        }
    }
}
