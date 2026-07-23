//! Combinatorial array generators: IPOG covering arrays and sequence covering
//! arrays, behind one CLI.
//!
//!   ipog ca      # reads ./ca.toml
//!   ipog sca 5   # reads ./sca.toml, strength 5 instead of the file's
//!
//! The config file for the chosen mode is required; `--strength` and
//! `--config` override what it says. See `config.rs`.

mod ca;
mod config;
mod sca;
mod util;

use clap::{Parser, ValueEnum};

use config::{CaConfig, ScaConfig};

#[derive(Copy, Clone, PartialEq, Eq, Debug, ValueEnum)]
enum Mode {
    /// t-way covering array over parameters x values (classic IPOG)
    Ca,
    /// t-way sequence covering array over events (IPOG adapted to sequences)
    Sca,
}

#[derive(Parser, Debug)]
#[command(name = "ipog", about = "IPOG covering / sequence covering array generator")]
struct Cli {
    /// Which array to build
    mode: Mode,

    /// Interaction strength t (overrides the config file)
    strength: Option<usize>,

    /// Interaction strength t, as a flag (wins over the positional form)
    #[arg(short = 't', long = "strength", value_name = "STRENGTH")]
    strength_flag: Option<usize>,

    /// Config file to read (defaults to ./ca.toml or ./sca.toml)
    #[arg(short = 'c', long)]
    config: Option<String>,

    /// PRNG seed for tie-breaking (overrides the config file)
    #[arg(short = 's', long)]
    seed: Option<u64>,

    /// ca: use the optimal orthogonal-array construction instead of IPOG
    #[arg(long)]
    oa: bool,
}

fn main() {
    let cli = Cli::parse();
    let override_t = cli.strength_flag.or(cli.strength);

    match cli.mode {
        Mode::Ca => {
            let cfg: CaConfig = config::load(cli.config.as_deref().unwrap_or("ca.toml"));
            let t = override_t.unwrap_or(cfg.strength);
            ca::run(cfg.params, &cfg.values, t, cli.seed.unwrap_or(cfg.seed), cli.oa)
        }
        Mode::Sca => {
            let cfg: ScaConfig = config::load(cli.config.as_deref().unwrap_or("sca.toml"));
            let t = override_t.unwrap_or(cfg.strength);
            sca::run(&cfg.events, t, cli.seed.unwrap_or(cfg.seed))
        }
    }
}
