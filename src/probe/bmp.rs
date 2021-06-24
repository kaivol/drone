// //! Black Magic Probe.
//
// use anyhow::Result;
// use tempfile::tempdir_in;
//
// use drone_config as config;
//
// use crate::{
//     cli::{FlashCmd, GdbCmd, LogCmd, ResetCmd},
//     color::Color,
//     log,
//     templates::Registry,
//     utils::{run_command, spawn_command},
// };
// use crate::utils::{SignalStream, WithSignals};
//
// use super::{
//     begin_log_output, gdb_script_command, run_gdb_client,
//     rustc_substitute_path,
// };
// use std::env;
//
// /// Runs `drone reset` command.
// pub async fn reset(
//     cmd: ResetCmd,
//     mut signals: SignalStream,
//     registry: Registry<'_>,
//     config: config::Config,
// ) -> Result<()> {
//     let ResetCmd {} = cmd;
//     let script = registry.bmp_reset(&config)?;
//     let gdb = gdb_script_command(&config, None, script.path());
//     run_command(gdb).with_signals(&mut signals, true).await
// }
//
// /// Runs `drone flash` command.
// pub async fn flash(
//     cmd: FlashCmd,
//     mut signals: SignalStream,
//     registry: Registry<'_>,
//     config: config::Config,
// ) -> Result<()> {
//     let FlashCmd { firmware } = cmd;
//     let script = registry.bmp_flash(&config)?;
//     let gdb = gdb_script_command(&config, Some(&firmware), script.path());
//     run_command(gdb).with_signals(&mut signals, true).await
// }
//
// /// Runs `drone gdb` command.
// pub async fn gdb(
//     cmd: GdbCmd,
//     mut signals: SignalStream,
//     registry: Registry<'_>,
//     config: config::Config,
// ) -> Result<()> {
//     let GdbCmd { firmware, reset, interpreter, gdb_args } = cmd;
//     let script = registry.bmp_gdb(&config, reset, &rustc_substitute_path().await?)?;
//     run_gdb_client(
//         &mut signals,
//         &config,
//         &gdb_args,
//         firmware.as_deref(),
//         interpreter.as_ref().map(String::as_ref),
//         script.path(),
//     ).await
// }
//
// /// Runs `drone log` command.
// pub async fn log_swo_serial(
//     cmd: LogCmd,
//     mut signals: SignalStream,
//     registry: Registry<'_>,
//     config: config::Config,
//     color: Color,
// ) -> Result<()> {
//     let LogCmd { reset, outputs } = cmd;
//     let config_log_swo = config.log.as_ref().unwrap().swo.as_ref().unwrap();
//     let serial_endpoint = config_log_swo.serial_endpoint.as_ref().unwrap();
//
//     // let dir = tempdir_in(env::temp_dir())?;
//     // let pipe = make_fifo(&dir, "pipe")?;
//     // let ports = outputs.iter().flat_map(|output| output.ports.iter().copied()).collect();
//     let script = registry.bmp_swo(&config, &ports, reset, &pipe)?;
//     // let mut gdb = spawn_command(gdb_script_command(&config, None, script.path()))?;
//
//     // let (pipe, packet) = gdb_script_wait(&mut signals, pipe).await?;
//     // setup_serial_endpoint(&mut signals, serial_endpoint, config_log_swo.baud_rate).await?;
//     // exhaust_fifo(serial_endpoint)?;
//     // log::capture(serial_endpoint.into(), log::Output::open_all(&outputs)?, log::swo::parser);
//     begin_log_output(color);
//     // gdb_script_continue(&mut signals, pipe, packet).await?;
//
//     // gdb.wait().with_signals(&mut signals, true).await?;
//     Ok(())
//     // block_with_signals(&mut signals, true, || async move {
//     //     gdb.wait().await?;
//     //     Ok(())
//     // }).await
// }
