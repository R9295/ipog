//! Bits shared by both generators: progress printing and combinatorics.

use std::io::{IsTerminal, Write};

pub struct Progress {
    pub tty: bool,
}

impl Progress {
    pub fn new() -> Self {
        Progress { tty: std::io::stdout().is_terminal() }
    }

    pub fn live(&self, msg: &str) {
        if self.tty {
            print!("\x1b[2K{msg}\r");
            let _ = std::io::stdout().flush();
        }
    }

    /// Clear the live line so ordinary output starts clean.
    pub fn clear(&self) {
        if self.tty {
            print!("\x1b[2K");
            let _ = std::io::stdout().flush();
        }
    }
}

/// SplitMix64 - a tiny PRNG so a seed reproduces a run exactly.
///
/// The algorithms below are greedy with many ties; the seed decides which of
/// the equally-good choices is taken, so different seeds give different (and
/// sometimes smaller) arrays for the same inputs.
///
/// Seed 0 is special: it disables randomization entirely and every tie falls
/// back to the fixed choice the caller names, which is the plain textbook IPOG.
pub struct Rng {
    state: u64,
    on: bool,
}

/// Which element a tie resolves to when randomization is off.
#[derive(Copy, Clone, PartialEq, Eq)]
pub enum Tie {
    First,
    Last,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng { state: seed, on: seed != 0 }
    }

    pub fn is_on(&self) -> bool {
        self.on
    }

    fn next_u64(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }

    /// Uniform-ish in `0..n`, or 0 when randomization is off.
    pub fn below(&mut self, n: usize) -> usize {
        if !self.on {
            return 0;
        }
        (self.next_u64() % n as u64) as usize
    }

    /// One of the equally-good `items`; `tie` says which one when off.
    pub fn pick<T: Copy>(&mut self, items: &[T], tie: Tie) -> T {
        if !self.on {
            return match tie {
                Tie::First => items[0],
                Tie::Last => items[items.len() - 1],
            };
        }
        items[self.below(items.len())]
    }

    pub fn shuffle<T>(&mut self, items: &mut [T]) {
        if !self.on {
            return;
        }
        for i in (1..items.len()).rev() {
            items.swap(i, self.below(i + 1));
        }
    }
}

pub fn perm(n: u128, k: u128) -> u128 {
    (0..k).map(|i| n - i).product()
}

/// Exact, comma-separated `nPk`, including values too large for `u128`.
///
/// This is used for informational search-space sizes. The generators' working
/// counts still use fixed-width integers because those counts must be
/// addressable in memory.
pub fn perm_commas(n: usize, k: usize) -> String {
    assert!(k <= n, "cannot choose {k} ordered items from {n}");

    // Little-endian base-10^9 limbs make comma formatting direct.
    const BASE: u128 = 1_000_000_000;
    let mut limbs = vec![1u32];
    for i in 0..k {
        let factor = (n - i) as u128;
        let mut carry = 0u128;
        for limb in &mut limbs {
            let product = *limb as u128 * factor + carry;
            *limb = (product % BASE) as u32;
            carry = product / BASE;
        }
        while carry != 0 {
            limbs.push((carry % BASE) as u32);
            carry /= BASE;
        }
    }

    let mut parts = limbs.iter().rev();
    let mut decimal = parts.next().unwrap().to_string();
    for part in parts {
        decimal.push_str(&format!("{part:09}"));
    }

    let mut out = String::with_capacity(decimal.len() + decimal.len() / 3);
    for (i, c) in decimal.chars().enumerate() {
        if i > 0 && (decimal.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

pub fn comb(n: u128, k: u128) -> u128 {
    let mut r = 1u128;
    for i in 0..k {
        r = r * (n - i) / (i + 1);
    }
    r
}

pub fn commas(x: u128) -> String {
    let s = x.to_string();
    let mut out = String::new();
    for (i, c) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::perm_commas;

    #[test]
    fn formats_permutations_beyond_u128() {
        assert_eq!(perm_commas(5, 3), "60");
        assert_eq!(
            perm_commas(72, 72),
            "61,234,458,376,886,086,861,524,070,385,274,672,740,778,091,784,697,328,983,823,014,963,978,384,987,221,689,274,204,160,000,000,000,000,000"
        );
    }
}
