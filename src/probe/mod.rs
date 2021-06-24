//! Debug probe interface.

use std::{
    convert::TryFrom,
    path::{Path},
};
use tokio::io::{BufReader};
// use std::process::Stdio;
use anyhow::{anyhow, bail, Error, Result};
use serde::{Deserialize, Serialize};
use tokio::process::Command;

use drone_config as config;

use crate::{
    cli::{FlashCmd, GdbCmd, LogCmd, ResetCmd},
    color::Color,
    templates::Registry,
    utils::{spawn_command},
};
use crate::utils::{SignalStream, run_command, WithSignals};
use tokio::io::{AsyncBufReadExt};
use std::process::Stdio;
use std::future::Future;
use std::ffi::OsString;

pub mod bmp;
pub mod jlink;
pub mod openocd;

/// An `enum` of all supported debug probes.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Probe {
    /// Black Magic Probe.
    Bmp,
    /// SEGGER J-Link.
    Jlink,
    /// OpenOCD.
    Openocd,
}

/// An `enum` of all supported debug loggers.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Log {
    /// ARM® SWO through debug probe.
    SwoProbe,
    /// ARM® SWO through USB-serial adapter.
    SwoSerial,
    /// Drone Serial Output through USB-serial adapter.
    DsoSerial,
}

impl<'a> TryFrom<&'a config::Config> for Probe {
    type Error = Error;

    fn try_from(config: &'a config::Config) -> Result<Self> {
        let config_probe = config
            .probe
            .as_ref()
            .ok_or_else(|| anyhow!("Missing `probe` section in `{}`", config::CONFIG_NAME))?;
        if config_probe.bmp.is_some() {
            bail!("BMP not supported");
        } else if config_probe.jlink.is_some() {
            bail!("JLink not supported");
        } else if config_probe.openocd.is_some() {
            Ok(Self::Openocd)
        } else {
            bail!(
                "Missing one of `probe.bmp`, `probe.jlink`, `probe.openocd` sections in `{}`",
                config::CONFIG_NAME
            );
        }
    }
}

impl<'a> TryFrom<&'a config::Config> for Log {
    type Error = Error;

    fn try_from(config: &'a config::Config) -> Result<Self> {
        let config_log = config
            .log
            .as_ref()
            .ok_or_else(|| anyhow!("Missing `log` section in `{}`", config::CONFIG_NAME))?;
        if let Some(config_log_swo) = &config_log.swo {
            if config_log_swo.serial_endpoint.is_some() {
                Ok(Self::SwoSerial)
            } else {
                Ok(Self::SwoProbe)
            }
        } else if config_log.dso.is_some() {
            Ok(Self::DsoSerial)
        } else {
            bail!("Missing one of `log.swo`, `log.dso` sections in `{}`", config::CONFIG_NAME);
        }
    }
}

impl Probe {
    /// Returns a function to serve `drone reset` command.
    #[allow(unused_variables)]
    pub(crate) async fn reset(
        self,
        cmd: ResetCmd,
        signals: SignalStream,
        registry: Registry<'_>,
        config: config::Config,
    ) -> Result<()> {
        match self {
            // Probe::Bmp => bmp::reset(cmd, signals, registry, config).await,
            // Probe::Jlink => jlink::reset(cmd, signals, registry, config).await,
            // Probe::Openocd => openocd::reset(cmd, signals, registry, config).await,
            _ => bail!("flash is unsupported")
        }
    }

    /// Returns a function to serve `drone flash` command.
    #[allow(unused_variables)]
    pub(crate) async fn flash(
        self,
        cmd: FlashCmd,
        signals: SignalStream,
        registry: Registry<'_>,
        config: config::Config,
    ) -> Result<()> {
        match self {
            // Probe::Bmp => bmp::flash(cmd, signals, registry, config).await,
            // Probe::Jlink => jlink::flash(cmd, signals, registry, config).await,
            // Probe::Openocd => openocd::flash(cmd, signals, registry, config).await,
            _ => bail!("flash is unsupported")
        }
    }

    /// Returns a function to serve `drone gdb` command.
    #[allow(unused_variables)]
    pub(crate) async fn gdb(
        self,
        cmd: GdbCmd,
        signals: SignalStream,
        registry: Registry<'_>,
        config: config::Config,
    ) -> Result<()> {
        match self {
            // Probe::Bmp => bmp::gdb(cmd, signals, registry, config).await,
            // Probe::Jlink => jlink::gdb(cmd, signals, registry, config).await,
            // Probe::Openocd => openocd::gdb(cmd, signals, registry, config).await,
            _ => bail!("flash is unsupported")
        }
    }
}

/// Returns a function to serve `drone log` command.
pub async fn log(
    probe: Probe,
    log: Log,
    cmd: LogCmd,
    signals: SignalStream,
    registry: Registry<'_>,
    config: config::Config,
    color: Color,
) -> Option<Result<()>> {
    match (probe, log) {
        // (Probe::Bmp, Log::SwoSerial) =>
        //     Some(bmp::log_swo_serial(cmd, signals, registry, config, color).await),
        // (Probe::Jlink, Log::DsoSerial) =>
        //     Some(jlink::log_dso_serial(cmd, signals, registry, config, color).await),
        (Probe::Openocd, Log::SwoProbe | Log::SwoSerial) =>
            Some(openocd::log_swo(cmd, signals, registry, config, color).await),
        _ => None,
    }
}

/// Returns whether or not the given configuration supports logging
pub fn supports_log(probe: Probe, log: Log) -> bool {
    match (probe, log) {
        // (Probe::Bmp, Log::SwoSerial) |
        // (Probe::Jlink, Log::DsoSerial) |
        (Probe::Openocd, Log::SwoProbe | Log::SwoSerial) => true,
        _ => false,
    }
}

/// Runs a GDB server.
pub async fn run_gdb_server<T>(
    mut gdb: Command,
    // interpreter: Option<&str>,
    f: impl Future<Output=Result<T>>,
) -> Result<T> {
    // if interpreter.is_some() {
    //     gdb.stdout(Stdio::piped());
    // }
    // detach_pgid(&mut gdb); // TODO
    let mut gdb = spawn_command(gdb)?;
    // if interpreter.is_some() {
    //     if let Some(stdout) = gdb.stdout.take() {
    //          tokio::spawn(async move {
    //             let stdout = BufReader::new(stdout);
    //             for line in  stdout.lines().next_line().await {
    //                 let mut line = line.expect("gdb-server stdout pipe fail");
    //                 line.push('\n');
    //                 println!("~{:?}", line);
    //             }
    //         });
    //     }
    // }
    let result = f.await?;
    gdb.kill().await.expect("gdb-server wasn't running");
    Ok(result)
}

/// Runs a GDB client.
pub async fn run_gdb_client(
    signals: &mut SignalStream,
    config: &config::Config,
    gdb_args: &[OsString],
    firmware: Option<&Path>,
    interpreter: Option<&str>,
    script: &Path,
) -> Result<()> {
    let mut gdb = Command::new(&config.probe.as_ref().unwrap().gdb_client_command);
    for arg in gdb_args {
        gdb.arg(arg);
    }
    if let Some(firmware) = firmware {
        gdb.arg(firmware);
    }
    gdb.arg("--command").arg(script);
    if let Some(interpreter) = interpreter {
        gdb.arg("--interpreter").arg(interpreter);
    }
    run_command(gdb).with_signals(signals, true).await
}

/// Creates a GDB script command.
pub fn gdb_script_command(
    config: &config::Config,
    firmware: Option<&Path>,
    script: &Path,
) -> Command {
    let mut gdb = Command::new(&config.probe.as_ref().unwrap().gdb_client_command);
    if let Some(firmware) = firmware {
        gdb.arg(firmware);
    }
    gdb.arg("--quiet");
    gdb.arg("--nx");
    gdb.arg("--batch");
    gdb.arg("--command").arg(script);
    gdb
}

/// Returns a GDB substitute-path for rustc sources.
pub async fn rustc_substitute_path() -> Result<String> {
    let mut rustc = Command::new("rustc");
    rustc.arg("--print").arg("sysroot");
    let sysroot = String::from_utf8(rustc.output().await?.stdout)?.trim().to_string();
    let mut rustc = Command::new("rustc");
    rustc.arg("--verbose");
    rustc.arg("--version");
    let commit_hash = String::from_utf8(rustc.output().await?.stdout)?
        .lines()
        .find_map(|line| {
            line.starts_with("commit-hash: ").then(|| line.splitn(2, ": ").nth(1).unwrap())
        })
        .ok_or_else(|| anyhow!("parsing of rustc output failed"))?
        .to_string();
    Ok(format!("/rustc/{} {}/lib/rustlib/src/rust", commit_hash, sysroot))
}
