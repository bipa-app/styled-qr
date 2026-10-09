use qrcode::Color as QrColor;
use qrcode::bits::Bits;
use qrcode::canvas::{Canvas, MaskPattern, is_functional};
use qrcode::types::QrError;

use crate::error::Error;

const FINDER_SPAN: u8 = 7;
const ALIGNMENT_RADIUS: u8 = 2;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum EcLevel {
    L,
    M,
    Q,
    #[default]
    H,
}

impl EcLevel {
    const fn to_qr(self) -> qrcode::EcLevel {
        match self {
            Self::L => qrcode::EcLevel::L,
            Self::M => qrcode::EcLevel::M,
            Self::Q => qrcode::EcLevel::Q,
            Self::H => qrcode::EcLevel::H,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Mask {
    Checkerboard,
    HorizontalLines,
    VerticalLines,
    DiagonalLines,
    LargeCheckerboard,
    Fields,
    Diamonds,
    Meadow,
}

impl Mask {
    pub const ALL: [Self; 8] = [
        Self::Checkerboard,
        Self::HorizontalLines,
        Self::VerticalLines,
        Self::DiagonalLines,
        Self::LargeCheckerboard,
        Self::Fields,
        Self::Diamonds,
        Self::Meadow,
    ];

    #[must_use]
    pub fn inverts(self, pos: Pos) -> bool {
        let (i, j) = (u32::from(pos.y), u32::from(pos.x));
        match self {
            Self::Checkerboard => (i + j) % 2 == 0,
            Self::HorizontalLines => i % 2 == 0,
            Self::VerticalLines => j % 3 == 0,
            Self::DiagonalLines => (i + j) % 3 == 0,
            Self::LargeCheckerboard => (i / 2 + j / 3) % 2 == 0,
            Self::Fields => (i * j) % 2 + (i * j) % 3 == 0,
            Self::Diamonds => ((i * j) % 2 + (i * j) % 3) % 2 == 0,
            Self::Meadow => ((i + j) % 2 + (i * j) % 3) % 2 == 0,
        }
    }

    const fn to_qr(self) -> MaskPattern {
        match self {
            Self::Checkerboard => MaskPattern::Checkerboard,
            Self::HorizontalLines => MaskPattern::HorizontalLines,
            Self::VerticalLines => MaskPattern::VerticalLines,
            Self::DiagonalLines => MaskPattern::DiagonalLines,
            Self::LargeCheckerboard => MaskPattern::LargeCheckerboard,
            Self::Fields => MaskPattern::Fields,
            Self::Diamonds => MaskPattern::Diamonds,
            Self::Meadow => MaskPattern::Meadow,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version(u8);

impl Version {
    pub const MIN: Self = Self(1);
    pub const MAX: Self = Self(40);

    pub fn new(number: u8) -> Result<Self, Error> {
        (1..=40)
            .contains(&number)
            .then_some(Self(number))
            .ok_or(Error::Version(number))
    }

    fn from_qr(version: qrcode::Version) -> Result<Self, Error> {
        match version {
            qrcode::Version::Normal(n) => u8::try_from(n)
                .ok()
                .filter(|n| (1..=40).contains(n))
                .map(Self)
                .ok_or(Error::Encode(QrError::InvalidVersion)),
            qrcode::Version::Micro(_) => Err(Error::Encode(QrError::InvalidVersion)),
        }
    }

    #[must_use]
    pub const fn number(self) -> u8 {
        self.0
    }

    #[must_use]
    pub const fn width(self) -> u8 {
        17 + 4 * self.0
    }

    #[must_use]
    pub fn alignment_coordinates(self) -> Vec<u8> {
        if self.0 < 2 {
            return Vec::new();
        }
        let count = self.0 / 7 + 2;
        let step = if self.0 == 32 {
            26
        } else {
            (self.0 * 4 + count * 2 + 1) / (count * 2 - 2) * 2
        };
        let mut coords = vec![6];
        let last = self.width() - FINDER_SPAN;
        coords.extend((0..count - 1).rev().map(|i| last - i * step));
        coords
    }

    fn qr(self) -> qrcode::Version {
        qrcode::Version::Normal(i16::from(self.0))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pos {
    pub x: u8,
    pub y: u8,
}

impl Pos {
    #[must_use]
    pub const fn new(x: u8, y: u8) -> Self {
        Self { x, y }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Role {
    Finder,
    Separator,
    Timing,
    Alignment,
    FormatInfo,
    VersionInfo,
    Data,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Module {
    pub dark: bool,
    pub role: Role,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Corner {
    TopLeft,
    TopRight,
    BottomLeft,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct CodewordLayout {
    codeword_of_module: Vec<Option<u16>>,
    block_of_codeword: Vec<u16>,
    capacity_per_block: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Symbol {
    version: Version,
    ec_level: EcLevel,
    mask: Mask,
    modules: Vec<Module>,
    codewords: CodewordLayout,
}

impl Symbol {
    pub fn encode(data: &[u8], ec_level: EcLevel) -> Result<Self, Error> {
        Self::encode_with(data, ec_level, Version::MIN)
    }

    pub fn encode_with(
        data: &[u8],
        ec_level: EcLevel,
        min_version: Version,
    ) -> Result<Self, Error> {
        Self::build(data, ec_level, min_version, None)
    }

    pub fn encode_masked(
        data: &[u8],
        ec_level: EcLevel,
        min_version: Version,
        mask: Mask,
    ) -> Result<Self, Error> {
        Self::build(data, ec_level, min_version, Some(mask))
    }

    fn build(
        data: &[u8],
        ec_level: EcLevel,
        min_version: Version,
        mask: Option<Mask>,
    ) -> Result<Self, Error> {
        let bits = encode_bits(data, ec_level, min_version)?;
        let version = Version::from_qr(bits.version())?;
        let raw = bits.into_bytes();
        let (data_codewords, ec_codewords) =
            qrcode::ec::construct_codewords(&raw, version.qr(), ec_level.to_qr())
                .map_err(Error::Encode)?;
        let mut canvas = Canvas::new(version.qr(), ec_level.to_qr());
        canvas.draw_all_functional_patterns();
        canvas.draw_data(&data_codewords, &ec_codewords);
        let centers = alignment_centers(version);
        let roles: Vec<(Pos, Role)> = positions(version.width())
            .map(|pos| (pos, role_of(version, &centers, pos)))
            .collect();
        let (mask, colors) = masked_colors(&canvas, mask, &roles);
        let modules: Vec<Module> = roles
            .iter()
            .zip(colors)
            .map(|(&(_, role), color)| Module {
                dark: color == QrColor::Dark,
                role,
            })
            .collect();
        let is_data: Vec<bool> = modules.iter().map(|m| m.role == Role::Data).collect();
        let codewords =
            CodewordLayout::compute(version, ec_level, &raw, ec_codewords.len(), &is_data)?;
        Ok(Self {
            version,
            ec_level,
            mask,
            modules,
            codewords,
        })
    }

    #[must_use]
    pub const fn mask(&self) -> Mask {
        self.mask
    }

    #[must_use]
    pub const fn version(&self) -> Version {
        self.version
    }

    #[must_use]
    pub const fn ec_level(&self) -> EcLevel {
        self.ec_level
    }

    #[must_use]
    pub const fn width(&self) -> u8 {
        self.version.width()
    }

    #[must_use]
    pub fn module(&self, pos: Pos) -> Option<Module> {
        self.index(pos).and_then(|i| self.modules.get(i).copied())
    }

    pub fn modules(&self) -> impl Iterator<Item = (Pos, Module)> + '_ {
        positions(self.width()).zip(self.modules.iter().copied())
    }

    #[must_use]
    pub fn finders(&self) -> [(Corner, Pos); 3] {
        let far = self.width() - FINDER_SPAN;
        [
            (Corner::TopLeft, Pos::new(0, 0)),
            (Corner::TopRight, Pos::new(far, 0)),
            (Corner::BottomLeft, Pos::new(0, far)),
        ]
    }

    #[must_use]
    pub fn alignment_centers(&self) -> Vec<Pos> {
        alignment_centers(self.version)
    }

    pub(crate) fn codeword_at(&self, pos: Pos) -> Option<(u16, u16)> {
        let codeword = self
            .index(pos)
            .and_then(|i| self.codewords.codeword_of_module.get(i).copied().flatten())?;
        let block = self
            .codewords
            .block_of_codeword
            .get(usize::from(codeword))
            .copied()?;
        Some((codeword, block))
    }

    pub(crate) const fn correctable_per_block(&self) -> u16 {
        self.codewords.capacity_per_block
    }

    fn index(&self, pos: Pos) -> Option<usize> {
        let w = self.width();
        (pos.x < w && pos.y < w).then(|| usize::from(pos.y) * usize::from(w) + usize::from(pos.x))
    }
}

fn masked_colors(
    canvas: &Canvas,
    mask: Option<Mask>,
    roles: &[(Pos, Role)],
) -> (Mask, Vec<QrColor>) {
    if let Some(mask) = mask {
        let mut masked = canvas.clone();
        masked.apply_mask(mask.to_qr());
        return (mask, masked.into_colors());
    }
    let unmasked = canvas.clone().into_colors();
    let best = canvas.apply_best_mask().into_colors();
    let flipped: Vec<(Pos, bool)> = roles
        .iter()
        .zip(unmasked.iter().zip(&best))
        .filter(|((_, role), _)| *role == Role::Data)
        .map(|((pos, _), (before, after))| (*pos, before != after))
        .collect();
    let chosen = Mask::ALL
        .into_iter()
        .find(|m| flipped.iter().all(|(pos, flip)| m.inverts(*pos) == *flip))
        .unwrap_or(Mask::Checkerboard);
    (chosen, best)
}

fn encode_bits(data: &[u8], ec_level: EcLevel, min_version: Version) -> Result<Bits, Error> {
    let auto = qrcode::bits::encode_auto(data, ec_level.to_qr()).map_err(Error::Encode)?;
    if Version::from_qr(auto.version())? >= min_version {
        return Ok(auto);
    }
    (min_version.0..=Version::MAX.0)
        .find_map(|n| {
            let mut bits = Bits::new(Version(n).qr());
            bits.push_optimal_data(data).ok()?;
            bits.push_terminator(ec_level.to_qr()).ok()?;
            Some(bits)
        })
        .ok_or(Error::Encode(QrError::DataTooLong))
}

fn positions(width: u8) -> impl Iterator<Item = Pos> {
    (0..width).flat_map(move |y| (0..width).map(move |x| Pos::new(x, y)))
}

fn alignment_centers(version: Version) -> Vec<Pos> {
    let coords = version.alignment_coordinates();
    let far = version.width() - FINDER_SPAN;
    coords
        .iter()
        .flat_map(|&y| coords.iter().map(move |&x| Pos::new(x, y)))
        .filter(|p| ![(6, 6), (6, far), (far, 6)].contains(&(p.x, p.y)))
        .collect()
}

fn role_of(version: Version, alignment_centers: &[Pos], pos: Pos) -> Role {
    let w = version.width();
    let in_corner = |span: u8| {
        let near = |v: u8| v < span;
        let far = |v: u8| v >= w - span;
        (near(pos.x) && (near(pos.y) || far(pos.y))) || (far(pos.x) && near(pos.y))
    };
    if in_corner(FINDER_SPAN) {
        return Role::Finder;
    }
    if in_corner(FINDER_SPAN + 1) {
        return Role::Separator;
    }
    if alignment_centers
        .iter()
        .any(|c| c.x.abs_diff(pos.x) <= ALIGNMENT_RADIUS && c.y.abs_diff(pos.y) <= ALIGNMENT_RADIUS)
    {
        return Role::Alignment;
    }
    if version.number() >= 7
        && ((pos.y < 6 && (w - 11..w - 8).contains(&pos.x))
            || (pos.x < 6 && (w - 11..w - 8).contains(&pos.y)))
    {
        return Role::VersionInfo;
    }
    if !is_functional(
        version.qr(),
        i16::from(w),
        i16::from(pos.x),
        i16::from(pos.y),
    ) {
        return Role::Data;
    }
    if pos.x == 6 || pos.y == 6 {
        return Role::Timing;
    }
    Role::FormatInfo
}

impl CodewordLayout {
    fn compute(
        version: Version,
        ec_level: EcLevel,
        raw: &[u8],
        ec_len: usize,
        is_data: &[bool],
    ) -> Result<Self, Error> {
        let data_blocks = data_codeword_blocks(version, ec_level, raw.len())?;
        let blocks = data_blocks.iter().max().map_or(1, |b| usize::from(*b) + 1);
        let ec_blocks = (0..ec_len).map(|k| u16::try_from(k % blocks).unwrap_or(u16::MAX));
        let block_of_codeword = data_blocks.iter().copied().chain(ec_blocks).collect();
        let codeword_of_module = codeword_zigzag(version.width(), is_data, raw.len() + ec_len);
        let max_errors = qrcode::ec::max_allowed_errors(version.qr(), ec_level.to_qr())
            .map_err(Error::Encode)?;
        let capacity_per_block = u16::try_from(max_errors / blocks).unwrap_or(u16::MAX);
        Ok(Self {
            codeword_of_module,
            block_of_codeword,
            capacity_per_block,
        })
    }
}

fn data_codeword_blocks(
    version: Version,
    ec_level: EcLevel,
    data_len: usize,
) -> Result<Vec<u16>, Error> {
    let probe = |digit: fn(usize) -> usize| -> Result<Vec<u8>, Error> {
        let raw: Vec<u8> = (0..data_len)
            .map(|i| u8::try_from(digit(i) % 256).unwrap_or(0))
            .collect();
        qrcode::ec::construct_codewords(&raw, version.qr(), ec_level.to_qr())
            .map(|(data, _)| data)
            .map_err(Error::Encode)
    };
    let low = probe(|i| i)?;
    let high = probe(|i| i / 256)?;
    let origin: Vec<usize> = low
        .iter()
        .zip(&high)
        .map(|(l, h)| usize::from(*h) * 256 + usize::from(*l))
        .collect();
    let blocks = origin
        .windows(2)
        .position(|w| w.get(1) <= w.first())
        .map_or(1, |p| p + 1);
    let starts: Vec<usize> = origin.iter().take(blocks).copied().collect();
    Ok(origin
        .iter()
        .map(|idx| {
            let block = starts
                .iter()
                .filter(|s| *s <= idx)
                .count()
                .saturating_sub(1);
            u16::try_from(block).unwrap_or(u16::MAX)
        })
        .collect())
}

fn codeword_zigzag(width: u8, is_data: &[bool], codewords: usize) -> Vec<Option<u16>> {
    let w = usize::from(width);
    let mut out = vec![None; w * w];
    let mut bit = 0usize;
    let mut upward = true;
    let mut right = w.saturating_sub(1);
    while right >= 1 {
        let rows: Vec<usize> = if upward {
            (0..w).rev().collect()
        } else {
            (0..w).collect()
        };
        for y in rows {
            for x in [right, right - 1] {
                let i = y * w + x;
                if !is_data.get(i).copied().unwrap_or(false) {
                    continue;
                }
                if let Some(slot) = out.get_mut(i).filter(|_| bit / 8 < codewords) {
                    *slot = u16::try_from(bit / 8).ok();
                }
                bit += 1;
            }
        }
        upward = !upward;
        right = match right.checked_sub(2) {
            Some(6) => 5,
            Some(next) => next,
            None => break,
        };
    }
    out
}

#[cfg(test)]
mod tests {
    use std::collections::{BTreeMap, BTreeSet};

    use super::*;

    const BITPLANES: u32 = 12;

    fn codeword_bitplanes(
        version: Version,
        ec_level: EcLevel,
        data_len: usize,
        ec_len: usize,
        is_data: &[bool],
    ) -> Vec<Option<u16>> {
        let draw = |byte: &dyn Fn(usize) -> u8| -> Vec<bool> {
            let data: Vec<u8> = (0..data_len).map(byte).collect();
            let ec: Vec<u8> = (data_len..data_len + ec_len).map(byte).collect();
            let mut canvas = Canvas::new(version.qr(), ec_level.to_qr());
            canvas.draw_all_functional_patterns();
            canvas.draw_data(&data, &ec);
            canvas
                .into_colors()
                .into_iter()
                .map(|c| c == QrColor::Dark)
                .collect()
        };
        let assigned = draw(&|_| 0xFF);
        let planes: Vec<Vec<bool>> = (0..BITPLANES)
            .map(|bit| draw(&|k| if (k >> bit) & 1 == 1 { 0xFF } else { 0 }))
            .collect();
        (0..assigned.len())
            .map(|i| {
                let is_data = is_data.get(i).copied().unwrap_or(false);
                let is_assigned = assigned.get(i).copied().unwrap_or(false);
                (is_data && is_assigned).then(|| {
                    planes
                        .iter()
                        .enumerate()
                        .filter(|(_, plane)| plane.get(i).copied().unwrap_or(false))
                        .map(|(bit, _)| 1u16 << bit)
                        .sum()
                })
            })
            .collect()
    }

    #[test]
    fn zigzag_placement_matches_the_reference_canvas() {
        for (len, ec) in [
            (0usize, EcLevel::H),
            (10, EcLevel::H),
            (120, EcLevel::Q),
            (700, EcLevel::H),
            (2900, EcLevel::L),
        ] {
            let data = vec![b'w'; len];
            let bits = qrcode::bits::encode_auto(&data, ec.to_qr()).expect("bits");
            let version = Version::from_qr(bits.version()).expect("version");
            let raw = bits.into_bytes();
            let (_, ec_codewords) =
                qrcode::ec::construct_codewords(&raw, version.qr(), ec.to_qr()).expect("ec");
            let centers = alignment_centers(version);
            let is_data: Vec<bool> = positions(version.width())
                .map(|p| role_of(version, &centers, p) == Role::Data)
                .collect();
            let reference =
                codeword_bitplanes(version, ec, raw.len(), ec_codewords.len(), &is_data);
            let zigzag = codeword_zigzag(version.width(), &is_data, raw.len() + ec_codewords.len());
            assert_eq!(zigzag, reference, "v{} {ec:?}", version.number());
        }
    }

    fn reference(data: &[u8], ec: EcLevel) -> qrcode::QrCode {
        qrcode::QrCode::with_error_correction_level(data, ec.to_qr()).expect("reference")
    }

    #[test]
    fn matches_reference_encoder_across_levels_and_sizes() {
        for ec in [EcLevel::L, EcLevel::M, EcLevel::Q, EcLevel::H] {
            for len in [0usize, 1, 17, 80, 300, 1000] {
                let data: Vec<u8> = (0..len).map(|i| b"bipa-qr-0123456789"[i % 18]).collect();
                let Ok(symbol) = Symbol::encode(&data, ec) else {
                    assert!(
                        qrcode::QrCode::with_error_correction_level(&data, ec.to_qr()).is_err()
                    );
                    continue;
                };
                let qr = reference(&data, ec);
                assert_eq!(usize::from(symbol.width()), qr.width());
                let dark: Vec<bool> = symbol.modules().map(|(_, m)| m.dark).collect();
                let expected: Vec<bool> =
                    qr.to_colors().iter().map(|c| *c == QrColor::Dark).collect();
                assert_eq!(dark, expected, "ec {ec:?} len {len}");
            }
        }
    }

    #[test]
    fn roles_cover_functional_patterns_for_every_version() {
        for n in 1..=40u8 {
            let version = Version(n);
            let w = version.width();
            let mut counts: BTreeMap<String, usize> = BTreeMap::new();
            for pos in positions(w) {
                let role = role_of(version, &alignment_centers(version), pos);
                let version_info = n >= 7
                    && ((pos.y < 6 && (w - 11..w - 8).contains(&pos.x))
                        || (pos.x < 6 && (w - 11..w - 8).contains(&pos.y)));
                let functional = version_info
                    || is_functional(
                        version.qr(),
                        i16::from(w),
                        i16::from(pos.x),
                        i16::from(pos.y),
                    );
                assert_eq!(role == Role::Data, !functional, "v{n} {pos:?} {role:?}");
                *counts.entry(format!("{role:?}")).or_default() += 1;
            }
            let alignments = alignment_centers(version).len();
            assert_eq!(counts.get("Finder").copied().unwrap_or(0), 3 * 49);
            assert_eq!(counts.get("Separator").copied().unwrap_or(0), 3 * 15);
            assert_eq!(
                counts.get("Alignment").copied().unwrap_or(0),
                25 * alignments
            );
            let version_info = if n >= 7 { 36 } else { 0 };
            assert_eq!(
                counts.get("VersionInfo").copied().unwrap_or(0),
                version_info
            );
            assert_eq!(counts.get("FormatInfo").copied().unwrap_or(0), 31);
        }
    }

    #[test]
    fn alignment_patterns_have_the_spec_shape() {
        for len in [20usize, 100, 400, 900] {
            let data = vec![b'7'; len];
            let symbol = Symbol::encode(&data, EcLevel::M).expect("encode");
            let centers = symbol.alignment_centers();
            assert_eq!(centers.is_empty(), symbol.version().number() == 1);
            for c in centers {
                for dy in 0..5u8 {
                    for dx in 0..5u8 {
                        let pos = Pos::new(c.x + dx - 2, c.y + dy - 2);
                        let ring = dx.abs_diff(2).max(dy.abs_diff(2));
                        let module = symbol.module(pos).expect("module");
                        assert_eq!(module.dark, ring != 1, "{pos:?}");
                        assert_eq!(module.role, Role::Alignment);
                    }
                }
            }
        }
    }

    #[test]
    fn alignment_coordinates_match_spec_table() {
        let table: [(u8, &[u8]); 6] = [
            (1, &[]),
            (2, &[6, 18]),
            (7, &[6, 22, 38]),
            (15, &[6, 26, 48, 70]),
            (32, &[6, 34, 60, 86, 112, 138]),
            (40, &[6, 30, 58, 86, 114, 142, 170]),
        ];
        for (n, expected) in table {
            assert_eq!(Version(n).alignment_coordinates(), expected, "v{n}");
        }
    }

    #[test]
    fn finders_sit_in_three_corners() {
        let symbol = Symbol::encode(b"finders", EcLevel::H).expect("encode");
        let far = symbol.width() - 7;
        assert_eq!(
            symbol.finders(),
            [
                (Corner::TopLeft, Pos::new(0, 0)),
                (Corner::TopRight, Pos::new(far, 0)),
                (Corner::BottomLeft, Pos::new(0, far)),
            ]
        );
        for (_, origin) in symbol.finders() {
            for dy in 0..7u8 {
                for dx in 0..7u8 {
                    let ring = dx.abs_diff(3).max(dy.abs_diff(3));
                    let module = symbol
                        .module(Pos::new(origin.x + dx, origin.y + dy))
                        .expect("module");
                    assert_eq!(module.dark, ring != 2);
                    assert_eq!(module.role, Role::Finder);
                }
            }
        }
        assert!(symbol.module(Pos::new(symbol.width(), 0)).is_none());
    }

    #[test]
    fn codeword_layout_partitions_data_modules() {
        for (len, ec) in [
            (10usize, EcLevel::H),
            (120, EcLevel::Q),
            (700, EcLevel::H),
            (2000, EcLevel::L),
        ] {
            let data = vec![b'q'; len];
            let symbol = Symbol::encode(&data, ec).expect("encode");
            let mut per_codeword: BTreeMap<u16, usize> = BTreeMap::new();
            let mut per_block: BTreeMap<u16, BTreeSet<u16>> = BTreeMap::new();
            let mut remainder = 0usize;
            for (pos, module) in symbol.modules() {
                match symbol.codeword_at(pos) {
                    Some((cw, block)) => {
                        assert_eq!(module.role, Role::Data);
                        *per_codeword.entry(cw).or_default() += 1;
                        per_block.entry(block).or_default().insert(cw);
                    }
                    None => remainder += usize::from(module.role == Role::Data),
                }
            }
            assert!(remainder <= 7, "remainder {remainder}");
            assert!(per_codeword.values().all(|n| *n == 8), "len {len}");
            let total = per_codeword.len();
            let sizes: Vec<usize> = per_block.values().map(BTreeSet::len).collect();
            assert_eq!(sizes.iter().sum::<usize>(), total);
            let (min, max) = (sizes.iter().min().copied(), sizes.iter().max().copied());
            assert!(
                max.zip(min).is_some_and(|(a, b)| a - b <= 1),
                "blocks {sizes:?}"
            );
            let max_errors =
                qrcode::ec::max_allowed_errors(symbol.version().qr(), ec.to_qr()).expect("ec");
            assert_eq!(
                usize::from(symbol.correctable_per_block()),
                max_errors / sizes.len()
            );
        }
    }

    #[test]
    fn block_assignment_matches_interleaving() {
        let symbol = Symbol::encode(&[b'z'; 400], EcLevel::H).expect("encode");
        let blocks: BTreeSet<u16> = symbol
            .modules()
            .filter_map(|(p, _)| symbol.codeword_at(p).map(|(_, b)| b))
            .collect();
        assert!(blocks.len() > 4);
        let cw0 = symbol
            .modules()
            .find_map(|(p, _)| symbol.codeword_at(p).filter(|(c, _)| *c == 0));
        let cw1 = symbol
            .modules()
            .find_map(|(p, _)| symbol.codeword_at(p).filter(|(c, _)| *c == 1));
        assert_eq!(cw0.map(|c| c.1), Some(0));
        assert_eq!(cw1.map(|c| c.1), Some(1));
    }

    #[test]
    fn minimum_version_pads_small_payloads() {
        let small = Symbol::encode(b"hi", EcLevel::H).expect("encode");
        assert_eq!(small.version().number(), 1);
        let min = Version::new(5).expect("version");
        let padded = Symbol::encode_with(b"hi", EcLevel::H, min).expect("encode");
        assert_eq!(padded.version(), min);
        assert_eq!(padded.width(), 37);
        let big = Symbol::encode_with(&[b'x'; 300], EcLevel::H, min).expect("encode");
        assert_eq!(
            big.version(),
            Symbol::encode(&[b'x'; 300], EcLevel::H)
                .expect("encode")
                .version()
        );
        assert!(Version::new(0).is_err());
        assert!(Version::new(41).is_err());
        assert_eq!(Version::new(40).ok(), Some(Version::MAX));
    }

    #[test]
    fn explicit_masks_are_applied_and_reported() {
        let data = b"https://bipa.app/mask";
        let best = Symbol::encode(data, EcLevel::H).expect("encode");
        assert_eq!(
            Symbol::encode_masked(data, EcLevel::H, Version::MIN, best.mask()).expect("masked"),
            best
        );
        let mut seen = BTreeSet::new();
        for mask in Mask::ALL {
            let symbol =
                Symbol::encode_masked(data, EcLevel::H, Version::MIN, mask).expect("masked");
            assert_eq!(symbol.mask(), mask);
            let bits: Vec<bool> = symbol.modules().map(|(_, m)| m.dark).collect();
            assert!(seen.insert(bits), "{mask:?} duplicates another mask");
            let functional_same = symbol
                .modules()
                .zip(best.modules())
                .filter(|((_, a), _)| {
                    matches!(a.role, Role::Finder | Role::Timing | Role::Alignment)
                })
                .all(|((_, a), (_, b))| a.dark == b.dark);
            assert!(functional_same);
        }
    }

    #[test]
    fn mask_formulas_match_the_reference_masking() {
        let data = &[b'm'; 90];
        let symbols: Vec<Symbol> = Mask::ALL
            .iter()
            .map(|m| Symbol::encode_masked(data, EcLevel::Q, Version::MIN, *m).expect("masked"))
            .collect();
        for (a, sa) in Mask::ALL.iter().zip(&symbols) {
            for (b, sb) in Mask::ALL.iter().zip(&symbols) {
                for ((pos, ma), (_, mb)) in sa.modules().zip(sb.modules()) {
                    if ma.role == Role::Data {
                        assert_eq!(
                            ma.dark != mb.dark,
                            a.inverts(pos) != b.inverts(pos),
                            "{a:?} {b:?} {pos:?}"
                        );
                    }
                }
            }
        }
        for len in [5usize, 60, 300, 900] {
            let data = vec![b'k'; len];
            let best = Symbol::encode(&data, EcLevel::H).expect("encode");
            let reference =
                qrcode::QrCode::with_error_correction_level(&data, qrcode::EcLevel::H).expect("qr");
            let masked = Symbol::encode_masked(&data, EcLevel::H, Version::MIN, best.mask())
                .expect("masked");
            assert_eq!(masked, best);
            let dark: Vec<bool> = best.modules().map(|(_, m)| m.dark).collect();
            let expected: Vec<bool> = reference
                .to_colors()
                .iter()
                .map(|c| *c == QrColor::Dark)
                .collect();
            assert_eq!(dark, expected);
        }
    }

    #[test]
    fn rejects_data_beyond_capacity() {
        assert!(matches!(
            Symbol::encode(&[b'x'; 3000], EcLevel::H),
            Err(Error::Encode(QrError::DataTooLong))
        ));
    }
}
