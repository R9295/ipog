# ipog

Generates small test sets that still cover every t-way interaction.

Two modes:

- **`ca`** — *covering array*. You have `n` parameters, each taking one of `v`
  values. Testing every combination means `v^n` runs. A t-way covering array is
  a much smaller set of rows in which, for **every** choice of `t` parameters,
  **every** one of the `v^t` value combinations shows up in some row.
- **`sca`** — *sequence covering array*. You have `n` events that can happen in
  any order. A t-way sequence covering array is a set of orderings in which
  every ordered t-tuple of distinct events appears (not necessarily adjacent) in
  at least one ordering.

Both use IPOG (Lei et al., *"IPOG: A General Strategy for T-Way Software
Testing"*), and both verify their own output before printing the summary.

## Usage

```
cargo run --release -- ca          # reads ./ca.toml,  writes ca_n5_v4_t3.csv
cargo run --release -- sca         # reads ./sca.toml, writes sca_n6_t3.csv
```

Options:

| Flag | Meaning |
| --- | --- |
| `<STRENGTH>` | positional; interaction strength `t`, overriding the config |
| `-t, --strength <N>` | same thing as a flag; wins over the positional |
| `-s, --seed <N>` | PRNG seed, overriding the config |
| `-c, --config <PATH>` | read this file instead of `ca.toml` / `sca.toml` |
| `--oa` | `ca` only: use the optimal orthogonal-array construction |
| `-h, --help` | full help |

Examples:

```
cargo run -- ca 2            # strength 2 instead of what ca.toml says
cargo run -- sca 4 -s 14     # strength 4, seed 14
cargo run -- ca --oa         # optimal array, when one exists for your n/v/t
```

The config file for the mode you pick is **required** — `ipog ca` with no
`ca.toml` in the working directory is an error.

## ca.toml

```toml
strength = 3                                    # t: interaction strength
params   = 5                                    # number of parameters
values   = ["skip", "vote", "notar", "final"]   # value labels; count = v
seed     = 0                                    # optional, defaults to 0
```

Output: `ca_n<params>_v<values>_t<strength>.csv`, one column per parameter
(`validator0`, `validator1`, ...) and one row per test, cells being your value
labels.

## sca.toml

```toml
strength = 3                                    # t: interaction strength
events   = ["propose", "vote", "cert",          # event names; count = n
            "finalize", "timeout", "repair"]
seed     = 0                                    # optional, defaults to 0
```

Output: `sca_n<events>_t<strength>.csv`, one column per position in the sequence
(`step0`, `step1`, ...) and one row per ordering, cells being your event names.

## About the seed

Both algorithms are greedy and hit a lot of ties. The seed decides which of the
equally-good choices is taken, so different seeds give different arrays — often
of different sizes.

**Seed 0 is special: it turns randomization off** and every tie falls back to a
fixed choice, i.e. plain textbook IPOG. That is the default because it is not
always worse — for the `ca.toml` above it gives 64 rows while no seed in 1..40
beats 85. For the `sca.toml` above at strength 4 it is the other way round: seed
0 gives 39 rows, seed 14 gives 37.

So: if the row count matters, try a handful of seeds and keep the best one.
A given seed always reproduces the same array exactly.

## License

GNU Affero General Public License v3.0 or later. See [LICENSE](LICENSE).
