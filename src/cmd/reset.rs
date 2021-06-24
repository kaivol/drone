//! `drone reset` command.

use crate::{cli::ResetCmd, probe::Probe, templates::Registry, utils::register_signals};
use anyhow::Result;
use drone_config as config;
use std::convert::TryFrom;

/// Runs `drone reset` command.
pub async fn run(cmd: ResetCmd) -> Result<()> {
    let signals = register_signals()?;
    let registry = Registry::new()?;
    let config = config::Config::read_from_current_dir()?;
    let probe = Probe::try_from(&config)?;
    probe.reset(cmd, signals, registry, config).await
}
