//! IPOG (the real one) - t-way *covering array* over parameters x values.
//!
//! n parameters (validators), each taking one of v values. A t-way covering
//! array is a set of rows such that for every choice of t validators, every one
//! of the v^t value combinations appears in some row.
//!
//! Algorithm (Lei et al., "IPOG: A General Strategy for T-Way Software Testing"):
//!   1. build the full cartesian product of the first t parameters (v^t rows);
//!   2. for each further parameter:
//!      a. horizontal growth - extend every existing row with the value that
//!         covers the most still-uncovered combinations involving that param;
//!      b. vertical growth - for each combination still uncovered, fold it into
//!         an existing row whose relevant cells are don't-care, else add a row.
//!
//! Only combinations involving the parameter being added are tracked: everything
//! among the earlier parameters is covered by induction.

use std::io::Write;
use std::time::Instant;

use crate::util::{comb, commas, Progress, Rng, Tie};

/// Cell value meaning "unconstrained" - free to be filled in later.
const DC: u8 = u8::MAX;

/// All k-subsets of `0..n`, ascending.
fn subsets(n: usize, k: usize) -> Vec<Vec<usize>> {
    let mut out = Vec::new();
    let mut cur = Vec::with_capacity(k);
    fn rec(n: usize, k: usize, start: usize, cur: &mut Vec<usize>, out: &mut Vec<Vec<usize>>) {
        if cur.len() == k {
            out.push(cur.clone());
            return;
        }
        for i in start..n {
            cur.push(i);
            rec(n, k, i + 1, cur, out);
            cur.pop();
        }
    }
    rec(n, k, 0, &mut cur, &mut out);
    out
}

/// Coverage of every t-way combination that involves the parameter being added.
///
/// Indexed as `subset_index * v^t + mixed_radix(values of subset, value of new param)`.
struct Coverage {
    subsets: Vec<Vec<usize>>,
    stride: usize,
    covered: Vec<bool>,
    remaining: usize,
    cursor: usize,
    v: usize,
    t: usize,
    param: usize,
}

impl Coverage {
    /// Combinations pairing `param` with every (t-1)-subset of `0..param`.
    fn new(param: usize, v: usize, t: usize) -> Self {
        let subsets = subsets(param, t - 1);
        let stride = v.pow(t as u32);
        let total = subsets.len() * stride;
        Coverage {
            subsets,
            stride,
            covered: vec![false; total],
            remaining: total,
            cursor: 0,
            v,
            t,
            param,
        }
    }

    /// Slot for one subset's cells plus the new parameter's value.
    fn index(&self, s: usize, cells: &[u8], nv: u8) -> usize {
        let mut idx = 0usize;
        for &p in &self.subsets[s] {
            idx = idx * self.v + cells[p] as usize;
        }
        s * self.stride + idx * self.v + nv as usize
    }

    /// How many new combinations `row` would cover if the new param were `nv`.
    fn gain(&self, row: &[u8], nv: u8) -> usize {
        (0..self.subsets.len())
            .filter(|&s| {
                self.subsets[s].iter().all(|&p| row[p] != DC)
                    && !self.covered[self.index(s, row, nv)]
            })
            .count()
    }

    /// Retire everything `row` covers (a no-op for cells left as don't-care).
    fn absorb(&mut self, row: &[u8]) {
        let nv = row[self.param];
        if nv == DC {
            return;
        }
        for s in 0..self.subsets.len() {
            if self.subsets[s].iter().any(|&p| row[p] == DC) {
                continue;
            }
            let idx = self.index(s, row, nv);
            if !self.covered[idx] {
                self.covered[idx] = true;
                self.remaining -= 1;
            }
        }
    }

    /// Next uncovered combination as (param, value) assignments.
    fn next_uncovered(&mut self) -> Option<Vec<(usize, u8)>> {
        while self.cursor < self.covered.len() && self.covered[self.cursor] {
            self.cursor += 1;
        }
        if self.cursor == self.covered.len() {
            return None;
        }
        let (s, mut rest) = (self.cursor / self.stride, self.cursor % self.stride);
        let mut digits = vec![0u8; self.t];
        for d in (0..self.t).rev() {
            digits[d] = (rest % self.v) as u8;
            rest /= self.v;
        }
        let mut out: Vec<(usize, u8)> =
            self.subsets[s].iter().copied().zip(digits.iter().copied()).collect();
        out.push((self.param, digits[self.t - 1]));
        Some(out)
    }
}

fn generate(n: usize, v: usize, t: usize, rng: &mut Rng, p: &Progress) -> Vec<Vec<u8>> {
    // start from the full cartesian product of the first t parameters
    let mut rows: Vec<Vec<u8>> = Vec::new();
    for i in 0..v.pow(t as u32) {
        let mut row = vec![DC; n];
        let mut rest = i;
        for d in (0..t).rev() {
            row[d] = (rest % v) as u8;
            rest /= v;
        }
        rows.push(row);
    }

    let mut ties: Vec<u8> = Vec::with_capacity(v);
    for i in t..n {
        let mut cov = Coverage::new(i, v, t);
        let combos = cov.remaining;

        // --- horizontal growth ---
        for r in 0..rows.len() {
            let gains: Vec<usize> = (0..v as u8).map(|nv| cov.gain(&rows[r], nv)).collect();
            let top = *gains.iter().max().unwrap();
            ties.clear();
            ties.extend((0..v as u8).filter(|&nv| gains[nv as usize] == top));
            rows[r][i] = rng.pick(&ties, Tie::Last);
            let row = std::mem::take(&mut rows[r]);
            cov.absorb(&row);
            rows[r] = row;
            if r % 64 == 0 || r + 1 == rows.len() {
                p.live(&format!(
                    "  [{:>2}/{n}] horizontal {:>6}/{} rows | uncovered {:>8}/{}",
                    i + 1,
                    r + 1,
                    rows.len(),
                    commas(cov.remaining as u128),
                    commas(combos as u128)
                ));
            }
        }
        let after_horizontal = rows.len();

        // --- vertical growth ---
        let mut slots: Vec<usize> = Vec::new();
        while let Some(assign) = cov.next_uncovered() {
            // reuse a row whose relevant cells are don't-care or already agree
            slots.clear();
            slots.extend(rows.iter().enumerate().filter_map(|(r, row)| {
                assign.iter().all(|&(pp, vv)| row[pp] == DC || row[pp] == vv).then_some(r)
            }));
            let r = if slots.is_empty() {
                rows.push(vec![DC; n]);
                rows.len() - 1
            } else {
                rng.pick(&slots, Tie::First)
            };
            let mut row = std::mem::take(&mut rows[r]);
            for &(pp, vv) in &assign {
                row[pp] = vv;
            }
            cov.absorb(&row);
            rows[r] = row;
            p.live(&format!(
                "  [{:>2}/{n}] vertical   +{:>5} rows       | uncovered {:>8}/{}",
                i + 1,
                rows.len() - after_horizontal,
                commas(cov.remaining as u128),
                commas(combos as u128)
            ));
        }

    }

    // any cell still unconstrained can take any value; pick one at random
    for row in rows.iter_mut() {
        for c in row.iter_mut() {
            if *c == DC {
                *c = rng.below(v) as u8;
            }
        }
    }

    rows
}

/// Independent check over every t-subset of parameters: how many combos are missed.
fn verify(rows: &[Vec<u8>], n: usize, v: usize, t: usize) -> usize {
    let mut missing = 0usize;
    let stride = v.pow(t as u32);
    let mut seen = vec![false; stride];
    for s in subsets(n, t) {
        seen.iter_mut().for_each(|b| *b = false);
        for row in rows {
            let mut idx = 0usize;
            for &p in &s {
                idx = idx * v + row[p] as usize;
            }
            seen[idx] = true;
        }
        missing += seen.iter().filter(|&&b| !b).count();
    }
    missing
}

fn is_prime(x: usize) -> bool {
    x >= 2 && (2..).take_while(|d| d * d <= x).all(|d| x % d != 0)
}

/// Optimal covering array from a linear MDS code, when one applies.
///
/// For prime `v` and `n <= v+1`, the extended Reed-Solomon code gives an
/// orthogonal array OA(v^t, v+1, v, t): every t-subset of columns sees all v^t
/// value combinations *exactly once*. That is the lower bound, so it is optimal
/// and nothing - IPOG included - can do better.
///
/// Row (c0..c_{t-1}) in GF(v)^t takes value sum(c_d * j^d) in column j, plus one
/// extra column c_{t-1}. Any t columns give a Vandermonde (or Vandermonde plus a
/// unit vector) coefficient matrix, which is nonsingular - hence strength t.
fn orthogonal_array(n: usize, v: usize, t: usize) -> Option<Vec<Vec<u8>>> {
    if !is_prime(v) || n > v + 1 || t > v || t < 1 {
        return None;
    }
    let mut rows = Vec::with_capacity(v.pow(t as u32));
    for i in 0..v.pow(t as u32) {
        let mut c = vec![0usize; t];
        let mut rest = i;
        for d in 0..t {
            c[d] = rest % v;
            rest /= v;
        }
        let row = (0..n)
            .map(|j| {
                if j < v {
                    let (mut acc, mut pow) = (0usize, 1usize);
                    for d in 0..t {
                        acc = (acc + c[d] * pow) % v;
                        pow = pow * j % v;
                    }
                    acc as u8
                } else {
                    c[t - 1] as u8
                }
            })
            .collect();
        rows.push(row);
    }
    Some(rows)
}

pub fn run(n: usize, values: &[String], t: usize, seed: u64, use_oa: bool) {
    let v = values.len();
    assert!(t >= 1 && t <= n, "strength {t} out of range for {n} parameters");
    assert!(v >= 1 && v < u8::MAX as usize, "value count {v} out of range");
    let p = Progress::new();
    let (nn, vv, tt) = (n as u128, v as u128, t as u128);

    let bar = "=".repeat(78);
    println!("{bar}");
    println!("IPOG covering array   {n} parameters (validators) x {v} values, strength t = {t}");
    println!("{bar}");
    let total = comb(nn, tt) * vv.pow(t as u32);
    println!("  comb({n}, {t})       = {:>22}   validator subsets", commas(comb(nn, tt)));
    println!("  {v}^{t}              = {:>22}   value combos per subset", commas(vv.pow(t as u32)));
    println!("  comb({n},{t}) * {v}^{t} = {:>22}   t-way combos to cover", commas(total));
    println!("  {v}^{n}             = {:>22}   exhaustive (all inputs)", commas(vv.pow(n as u32)));
    println!(
        "  perm({n}, {n})      = {:>22}   orderings of one row (if sequence matters)",
        commas((1..=nn).product::<u128>())
    );
    if !use_oa && orthogonal_array(n, v, t).is_some() {
        println!("  note                = {v} is prime and {n} <= {v}+1, so an orthogonal array hits");
        println!("                        the {} row lower bound - run with --oa for the optimum",
            commas(vv.pow(t as u32)));
    }

    let clock = Instant::now();
    let mut rng = Rng::new(seed);
    let rows = if use_oa {
        orthogonal_array(n, v, t)
            .expect("no MDS construction for this n/v/t - drop --oa to use IPOG")
    } else {
        generate(n, v, t, &mut rng, &p)
    };
    let elapsed = clock.elapsed().as_secs_f64();

    p.clear();
    let missing = verify(&rows, n, v, t);

    let path = format!("ca_n{n}_v{v}_t{t}{}.csv", if use_oa { "_oa" } else { "" });
    {
        let mut f = std::io::BufWriter::new(std::fs::File::create(&path).unwrap());
        let header: Vec<String> = (0..n).map(|i| format!("validator{i}")).collect();
        writeln!(f, "{}", header.join(",")).unwrap();
        for row in &rows {
            let cells: Vec<&str> = row.iter().map(|&c| values[c as usize].as_str()).collect();
            writeln!(f, "{}", cells.join(",")).unwrap();
        }
    }

    println!("{bar}");
    println!("  rows            : {}", commas(rows.len() as u128));
    println!(
        "  seed            : {}",
        if use_oa {
            "n/a".to_string()
        } else if rng.is_on() {
            seed.to_string()
        } else {
            "0 (deterministic - no random tie-breaking)".to_string()
        }
    );
    println!("  combos covered  : {} / {}", commas(total - missing as u128), commas(total));
    println!(
        "  verification    : {}",
        if missing == 0 {
            "OK - all t-way combos covered".to_string()
        } else {
            format!("FAILED - {} missing", commas(missing as u128))
        }
    );
    println!("  vs exhaustive   : {:.3e} x smaller", vv.pow(n as u32) as f64 / rows.len() as f64);
    println!("  wall clock      : {elapsed:.3}s");
    println!("  written         : {path}");
    println!("{bar}");
}
