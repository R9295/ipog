//! IPOG-style generator for t-way *sequence* covering arrays (SCA).
//!
//! A t-way SCA over n events is a set of sequences (each a permutation of all n
//! events) such that every ordered t-tuple of distinct events appears as a
//! subsequence of at least one row. The target count is perm(n, t) ordered
//! tuples = comb(n, t) unordered subsets x t! orderings each.
//!
//! The algorithm is IPOG adapted to sequences:
//!   * horizontal growth - the new event is inserted into every existing row at
//!     the position that covers the most still-uncovered tuples;
//!   * vertical growth   - while tuples remain uncovered, seed a fresh row with
//!     one of them and greedily insert the other events.
//!
//! Tuples are addressed by mixed-radix index (e0*n^(t-1) + e1*n^(t-2) + ...),
//! so coverage is a flat bit table rather than a hash set.

use std::io::Write;
use std::time::Instant;

use crate::util::{comb, commas, perm, Progress, Rng, Tie};

/// Enumerate every k-combination (order preserved) of `items[start..]`, feeding
/// each one to `f` as a mixed-radix index accumulated onto `acc`.
fn walk<F: FnMut(usize)>(items: &[usize], k: usize, start: usize, acc: usize, n: usize, f: &mut F) {
    if k == 0 {
        f(acc);
        return;
    }
    if start + k > items.len() {
        return;
    }
    for i in start..=items.len() - k {
        walk(items, k - 1, i + 1, acc * n + items[i], n, f);
    }
}

/// Every t-subsequence of `prefix ++ [e] ++ suffix` that contains `e`.
fn each_new<F: FnMut(usize)>(
    prefix: &[usize],
    e: usize,
    suffix: &[usize],
    t: usize,
    n: usize,
    f: &mut F,
) {
    for i in 0..t {
        let j = t - 1 - i;
        if i > prefix.len() || j > suffix.len() {
            continue;
        }
        walk(prefix, i, 0, 0, n, &mut |base: usize| {
            walk(suffix, j, 0, base * n + e, n, f);
        });
    }
}

/// Every ordered k-selection of `items`, as a slice handed to `f`.
fn each_perm<F: FnMut(&[usize])>(
    items: &[usize],
    k: usize,
    used: &mut Vec<bool>,
    buf: &mut Vec<usize>,
    f: &mut F,
) {
    if buf.len() == k {
        f(buf);
        return;
    }
    for i in 0..items.len() {
        if used[i] {
            continue;
        }
        used[i] = true;
        buf.push(items[i]);
        each_perm(items, k, used, buf, f);
        buf.pop();
        used[i] = false;
    }
}

struct Coverage {
    /// `needed[idx]` = tuple is in scope for the active events and still uncovered.
    needed: Vec<bool>,
    remaining: usize,
    n: usize,
    t: usize,
}

impl Coverage {
    fn new(n: usize, t: usize) -> Self {
        Coverage { needed: vec![false; n.pow(t as u32)], remaining: 0, n, t }
    }

    /// Bring every tuple over `active` that involves `e` into scope.
    fn activate(&mut self, active: &[usize], e: usize) -> usize {
        let (n, t) = (self.n, self.t);
        let others: Vec<usize> = active.iter().copied().filter(|&x| x != e).collect();
        let mut used = vec![false; others.len()];
        let mut buf = Vec::with_capacity(t);
        let mut added = 0;
        each_perm(&others, t - 1, &mut used, &mut buf, &mut |perm: &[usize]| {
            for pos in 0..t {
                let mut idx = 0usize;
                for d in 0..t {
                    let ev = match d.cmp(&pos) {
                        std::cmp::Ordering::Less => perm[d],
                        std::cmp::Ordering::Equal => e,
                        std::cmp::Ordering::Greater => perm[d - 1],
                    };
                    idx = idx * n + ev;
                }
                if !self.needed[idx] {
                    self.needed[idx] = true;
                    added += 1;
                }
            }
        });
        self.remaining += added;
        added
    }

    fn mark(&mut self, idx: usize) {
        if self.needed[idx] {
            self.needed[idx] = false;
            self.remaining -= 1;
        }
    }

    /// Mark every t-subsequence of a whole row.
    fn mark_row(&mut self, row: &[usize]) {
        let (n, t) = (self.n, self.t);
        let mut hits = Vec::new();
        walk(row, t, 0, 0, n, &mut |idx| hits.push(idx));
        for idx in hits {
            self.mark(idx);
        }
    }

    /// Position for `e` in `row` that newly covers the most tuples; ties (and a
    /// row where every position gains nothing) are broken by `rng`.
    fn best_insertion(&self, row: &[usize], e: usize, rng: &mut Rng) -> usize {
        let (n, t) = (self.n, self.t);
        let (mut best, mut ties) = (0usize, Vec::new());
        for p in 0..=row.len() {
            let mut gain = 0usize;
            each_new(&row[..p], e, &row[p..], t, n, &mut |idx| {
                if self.needed[idx] {
                    gain += 1;
                }
            });
            if gain > best {
                best = gain;
                ties.clear();
            }
            if gain == best {
                ties.push(p);
            }
        }
        rng.pick(&ties, Tie::First)
    }

    /// Insert `e` at `pos` in `row` and retire the tuples it covers.
    fn insert_at(&mut self, row: &mut Vec<usize>, e: usize, pos: usize) {
        let (n, t) = (self.n, self.t);
        let mut hits = Vec::new();
        each_new(&row[..pos], e, &row[pos..], t, n, &mut |idx| hits.push(idx));
        for idx in hits {
            self.mark(idx);
        }
        row.insert(pos, e);
    }

    fn decode(&self, mut idx: usize) -> Vec<usize> {
        let mut digits = vec![0usize; self.t];
        for d in (0..self.t).rev() {
            digits[d] = idx % self.n;
            idx /= self.n;
        }
        digits
    }

    fn first_needed(&self) -> Option<usize> {
        self.needed.iter().position(|&b| b)
    }
}

fn generate(
    n: usize,
    t: usize,
    events: &[String],
    rng: &mut Rng,
    p: &Progress,
) -> Vec<Vec<usize>> {
    let mut cov = Coverage::new(n, t);
    let mut active: Vec<usize> = (0..t).collect();

    // start from all t! permutations of the first t events
    let mut rows: Vec<Vec<usize>> = Vec::new();
    {
        let mut used = vec![false; t];
        let mut buf = Vec::with_capacity(t);
        each_perm(&active.clone(), t, &mut used, &mut buf, &mut |perm: &[usize]| {
            rows.push(perm.to_vec());
        });
    }
    for e in 0..t {
        cov.activate(&active, e);
    }
    let seeded: Vec<Vec<usize>> = rows.clone();
    for row in &seeded {
        cov.mark_row(row);
    }
    rng.shuffle(&mut rows);

    for k in t..n {
        let e = k;
        active.push(e);
        cov.activate(&active, e);

        // --- horizontal growth ---
        for i in 0..rows.len() {
            let mut row = std::mem::take(&mut rows[i]);
            let pos = cov.best_insertion(&row, e, rng);
            cov.insert_at(&mut row, e, pos);
            rows[i] = row;
            p.live(&format!(
                "  [{:>2}/{n}] {:<12} horizontal {:>4}/{} rows | uncovered {:>7}",
                k + 1,
                events[e],
                i + 1,
                rows.len(),
                cov.remaining
            ));
        }
        let after_horizontal = rows.len();

        // --- vertical growth ---
        while let Some(idx) = cov.first_needed() {
            let seed = cov.decode(idx);
            let mut row = seed.clone();
            let mut rest: Vec<usize> =
                active.iter().copied().filter(|o| !seed.contains(o)).collect();
            rng.shuffle(&mut rest);
            for other in rest {
                let pos = cov.best_insertion(&row, other, rng);
                cov.insert_at(&mut row, other, pos);
            }
            cov.mark_row(&row);
            rows.push(row);
            p.live(&format!(
                "  [{:>2}/{n}] {:<12} vertical   +{:>3} rows      | uncovered {:>7}",
                k + 1,
                events[e],
                rows.len() - after_horizontal,
                cov.remaining
            ));
        }

    }

    rows
}

/// Independent check: how many of the perm(n, t) ordered tuples are covered.
fn verify(rows: &[Vec<usize>], n: usize, t: usize) -> usize {
    let mut seen = vec![false; n.pow(t as u32)];
    for row in rows {
        walk(row, t, 0, 0, n, &mut |idx| seen[idx] = true);
    }
    let mut missing = 0usize;
    for (idx, &hit) in seen.iter().enumerate() {
        let mut digits = Vec::with_capacity(t);
        let mut rest = idx;
        for _ in 0..t {
            digits.push(rest % n);
            rest /= n;
        }
        let distinct = {
            let mut d = digits.clone();
            d.sort_unstable();
            d.dedup();
            d.len() == t
        };
        if distinct && !hit {
            missing += 1;
        }
    }
    missing
}

pub fn run(events: &[String], t: usize, seed: u64) {
    let n = events.len();
    assert!(t >= 1 && t <= n, "strength {t} out of range for {n} events");
    let p = Progress::new();
    let (nn, tt) = (n as u128, t as u128);

    let bar = "=".repeat(72);
    println!("{bar}");
    println!("IPOG sequence covering array   n = {n} events, strength t = {t}");
    println!("{bar}");
    println!("  events: {}", events.join(" "));
    println!("{bar}");
    println!("  comb({n}, {t}) = {:>12}   distinct event subsets", commas(comb(nn, tt)));
    println!("  perm({t}, {t}) = {:>12}   orderings per subset", commas(perm(tt, tt)));
    println!("  perm({n}, {t}) = {:>12}   ordered tuples to cover", commas(perm(nn, tt)));
    println!("  perm({n}, {n}) = {:>12}   possible rows (search space)", commas(perm(nn, nn)));
    println!(
        "  each row of length {n} covers up to comb({n}, {t}) = {} tuples",
        commas(comb(nn, tt))
    );

    let clock = Instant::now();
    let mut rng = Rng::new(seed);
    let rows = generate(n, t, events, &mut rng, &p);
    let elapsed = clock.elapsed().as_secs_f64();

    p.clear();
    let missing = verify(&rows, n, t);

    // the array itself goes to a file - one column per position in the sequence
    let path = format!("sca_n{n}_t{t}.csv");
    {
        let mut f = std::io::BufWriter::new(std::fs::File::create(&path).unwrap());
        let header: Vec<String> = (0..n).map(|i| format!("step{i}")).collect();
        writeln!(f, "{}", header.join(",")).unwrap();
        for row in &rows {
            let cells: Vec<&str> = row.iter().map(|&e| events[e].as_str()).collect();
            writeln!(f, "{}", cells.join(",")).unwrap();
        }
    }

    let total = perm(nn, tt);
    println!("{bar}");
    println!("  rows           : {}", rows.len());
    println!(
        "  seed           : {}",
        if rng.is_on() { seed.to_string() } else { "0 (deterministic - no random tie-breaking)".into() }
    );
    println!("  tuples covered : {} / {}", commas(total - missing as u128), commas(total));
    println!(
        "  verification   : {}",
        if missing == 0 {
            "OK - all tuples covered".to_string()
        } else {
            format!("FAILED - {missing} missing")
        }
    );
    println!("  wall clock     : {elapsed:.3}s");
    println!("  written        : {path}");
    println!("{bar}");
}
