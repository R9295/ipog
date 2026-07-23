//! TOML configuration, one file per mode: `ca.toml` for covering arrays,
//! `sca.toml` for sequence covering arrays. The file for the selected mode is
//! required; `--config <path>` points at a different one.
//!
//! ```toml
//! # ca.toml
//! strength = 3
//! params   = 5
//! values   = ["skip", "vote", "notar", "final"]
//! seed     = 0   # optional
//!
//! # sca.toml
//! strength = 3
//! events   = ["propose", "vote", "cert", "finalize", "timeout", "repair"]
//! seed     = 0   # optional
//! ```

use serde::Deserialize;

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct CaConfig {
    /// Interaction strength t.
    pub strength: usize,
    /// Number of parameters.
    pub params: usize,
    /// Value labels; their count is the number of values per parameter.
    pub values: Vec<String>,
    /// PRNG seed for tie-breaking. Same seed + same inputs = same array.
    #[serde(default)]
    pub seed: u64,
}

#[derive(Deserialize, Debug)]
#[serde(deny_unknown_fields)]
pub struct ScaConfig {
    /// Interaction strength t.
    pub strength: usize,
    /// Event names, in index order; their count is the event count.
    pub events: Vec<String>,
    /// PRNG seed for tie-breaking. Same seed + same inputs = same array.
    #[serde(default)]
    pub seed: u64,
}

/// Read and parse `path`, exiting with a plain message if either step fails.
pub fn load<T: serde::de::DeserializeOwned>(path: &str) -> T {
    let text = std::fs::read_to_string(path).unwrap_or_else(|e| {
        eprintln!("ipog: cannot read {path}: {e}");
        std::process::exit(1);
    });
    toml::from_str(&text).unwrap_or_else(|e| {
        eprintln!("ipog: {path}: {e}");
        std::process::exit(1);
    })
}
