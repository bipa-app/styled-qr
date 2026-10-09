use std::collections::{BTreeMap, BTreeSet};

use crate::symbol::{Pos, Symbol};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BudgetUsage {
    worst_block: u16,
    correctable: u16,
}

impl BudgetUsage {
    #[must_use]
    pub fn of(symbol: &Symbol, hidden: impl IntoIterator<Item = Pos>) -> Self {
        let mut per_block: BTreeMap<u16, BTreeSet<u16>> = BTreeMap::new();
        for (codeword, block) in hidden.into_iter().filter_map(|p| symbol.codeword_at(p)) {
            per_block.entry(block).or_default().insert(codeword);
        }
        let worst_block = per_block
            .values()
            .map(|c| u16::try_from(c.len()).unwrap_or(u16::MAX))
            .max()
            .unwrap_or(0);
        Self {
            worst_block,
            correctable: symbol.correctable_per_block(),
        }
    }

    #[must_use]
    pub const fn worst_block(self) -> u16 {
        self.worst_block
    }

    #[must_use]
    pub const fn correctable(self) -> u16 {
        self.correctable
    }

    #[must_use]
    pub fn fraction(self) -> f64 {
        if self.correctable == 0 {
            f64::from(u8::from(self.worst_block > 0))
        } else {
            f64::from(self.worst_block) / f64::from(self.correctable)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::symbol::{EcLevel, Role};

    #[test]
    fn counts_distinct_codewords_in_the_worst_block() {
        let symbol = Symbol::encode(&[b'k'; 200], EcLevel::H).expect("encode");
        assert_eq!(BudgetUsage::of(&symbol, []).worst_block(), 0);
        assert!((BudgetUsage::of(&symbol, []).fraction() - 0.0).abs() < 1e-12);
        let first: Vec<Pos> = symbol
            .modules()
            .filter(|(p, _)| symbol.codeword_at(*p).is_some_and(|(c, _)| c == 0))
            .map(|(p, _)| p)
            .collect();
        assert_eq!(first.len(), 8);
        let usage = BudgetUsage::of(&symbol, first.iter().copied());
        assert_eq!(usage.worst_block(), 1);
        assert_eq!(usage.correctable(), symbol.correctable_per_block());
        let functional: Vec<Pos> = symbol
            .modules()
            .filter(|(_, m)| m.role != Role::Data)
            .map(|(p, _)| p)
            .collect();
        assert_eq!(BudgetUsage::of(&symbol, functional).worst_block(), 0);
    }

    #[test]
    fn fraction_handles_blocks_without_correction_capacity() {
        let usage = |worst_block, correctable| BudgetUsage {
            worst_block,
            correctable,
        };
        assert!((usage(0, 0).fraction() - 0.0).abs() < 1e-12);
        assert!((usage(1, 0).fraction() - 1.0).abs() < 1e-12);
        assert!((usage(3, 4).fraction() - 0.75).abs() < 1e-12);
    }

    #[test]
    fn hiding_everything_exceeds_the_budget() {
        let symbol = Symbol::encode(b"https://bipa.app", EcLevel::H).expect("encode");
        let usage = BudgetUsage::of(&symbol, symbol.modules().map(|(p, _)| p));
        assert!(usage.fraction() > 1.0);
    }
}
