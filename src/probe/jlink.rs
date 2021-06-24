// //! Segger J-Link.
//
// // use signal_hook::iterator::Signals;
// use std::{path::Path, env};
//
// use anyhow::Result;
// use tempfile::tempdir_in;
// use tokio::process::Command;
//
// use drone_config as config;
//
// use crate::{
//     cli::{FlashCmd, GdbCmd, LogCmd, ResetCmd},
//     color::Color,
//     log,
//     templates::Registry,
//     utils::{
//         run_command, search_rust_tool, spawn_command,
//     },
// };
// use crate::utils::{SignalStream, WithSignals};
//
// use super::{
//     begin_log_output, gdb_script_command, run_gdb_client,
//     run_gdb_server, rustc_substitute_path,
// };
//
// /// Runs `drone reset` command.
// pub async fn reset(
//     cmd: ResetCmd,
//     mut signals: SignalStream,
//     registry: Registry<'_>,
//     config: config::Config,
// ) -> Result<()> {
//     let ResetCmd {} = cmd;
//     let config_probe_jlink = config.probe.as_ref().unwrap().jlink.as_ref().unwrap();
//     let script = registry.jlink_reset()?;
//     let mut commander = Command::new(&config_probe_jlink.commander_command);
//     jlink_args(&mut commander, config_probe_jlink);
//     commander_script(&mut commander, script.path());
//     run_command(commander).with_signals(&mut signals, true).await;
//     Ok(())
// }
//
// /// Runs the command.
// pub async fn flash(
//     cmd: FlashCmd,
//     mut signals: SignalStream,
//     registry: Registry<'_>,
//     config: config::Config,
// ) -> Result<()> {
//     let FlashCmd { firmware } = cmd;
//     let config_probe_jlink = config.probe.as_ref().unwrap().jlink.as_ref().unwrap();
//     let firmware_bin = &firmware.with_extension("bin");
//     let script = registry.jlink_flash(firmware_bin, config.memory.flash.origin)?;
//
//     let mut objcopy = Command::new(search_rust_tool("llvm-objcopy").await?);
//     objcopy.arg(firmware);
//     objcopy.arg(firmware_bin);
//     objcopy.arg("--output-target=binary");
//     run_command(objcopy).with_signals(&mut signals, true).await?;
//     #[cfg(unix)] {
//         use std::fs;
//         use std::os::unix::fs::PermissionsExt;
//         fs::set_permissions(firmware_bin, fs::Permissions::from_mode(0o644))?;
//     }
//
//     let mut commander = Command::new(&config_probe_jlink.commander_command);
//     jlink_args(&mut commander, config_probe_jlink);
//     commander_script(&mut commander, script.path());
//     run_command(commander).with_signals(&mut signals, true).await?;
//     Ok(())
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
//     let config_probe_jlink = config.probe.as_ref().unwrap().jlink.as_ref().unwrap();
//
//     let mut gdb_server = Command::new(&config_probe_jlink.gdb_server_command);
//     jlink_args(&mut gdb_server, config_probe_jlink);
//     gdb_server_args(&mut gdb_server, config_probe_jlink);
//     let _gdb_server = run_gdb_server(
//         gdb_server,
//         interpreter.as_ref().map(String::as_ref),
//         async {
//             let script = registry.jlink_gdb(&config, reset, &rustc_substitute_path().await?)?;
//             run_gdb_client(
//                 &mut signals,
//                 &config,
//                 &gdb_args,
//                 firmware.as_deref(),
//                 interpreter.as_ref().map(String::as_ref),
//                 script.path(),
//             ).await?;
//             Ok(())
//         }
//     ).await?;
//     Ok(())
// }
//
// /// Runs `drone log` command.
// pub async fn log_dso_serial(
//     cmd: LogCmd,
//     mut signals: SignalStream,
//     registry: Registry<'_>,
//     config: config::Config,
//     color: Color,
// ) -> Result<()> {
//     let LogCmd { reset, outputs } = cmd;
//     let config_probe_jlink = config.probe.as_ref().unwrap().jlink.as_ref().unwrap();
//     let config_log_dso = config.log.as_ref().unwrap().dso.as_ref().unwrap();
//
//     let mut gdb_server = Command::new(&config_probe_jlink.gdb_server_command);
//     jlink_args(&mut gdb_server, config_probe_jlink);
//     gdb_server_args(&mut gdb_server, config_probe_jlink);
//     run_gdb_server(gdb_server, None, async {
//         let dir = tempdir_in(env::temp_dir())?;
//         // let pipe = make_fifo(&dir, "pipe")?;
//         // let ports = outputs.iter().flat_map(|output| output.ports.iter().copied()).collect();
//         // let script = registry.jlink_dso(&config, &ports, reset, &pipe)?;
//         // let mut gdb = spawn_command(gdb_script_command(&config, None, script.path()))?;
//
//         // let (pipe, packet) = gdb_script_wait(&mut signals, pipe).await?;
//         // setup_serial_endpoint(&mut signals, &config_log_dso.serial_endpoint, config_log_dso.baud_rate).await?;
//         // exhaust_fifo(&config_log_dso.serial_endpoint)?;
//         // log::capture(
//         //     config_log_dso.serial_endpoint.clone().into(),
//         //     log::Output::open_all(&outputs)?,
//         //     log::dso::parser,
//         // );
//         begin_log_output(color);
//         // gdb_script_continue(&mut signals, pipe, packet).await?;
//
//         // block_with_signals(&mut signals, true, || async move {
//         //     gdb.wait().await?;
//         //     Ok(())
//         // }).await;
//         Ok(())
//     }).await?;
//     Ok(())
// }
//
// fn jlink_args(jlink: &mut Command, config_probe_jlink: &config::ProbeJlink) {
//     jlink.arg("-Device").arg(&config_probe_jlink.device);
//     jlink.arg("-Speed").arg(config_probe_jlink.speed.to_string());
//     jlink.arg("-If").arg(&config_probe_jlink.interface);
//     if config_probe_jlink.interface == "JTAG" {
//         jlink.arg("-JTAGConf").arg("-1,-1");
//     }
// }
//
// fn gdb_server_args(gdb_server: &mut Command, config_probe_jlink: &config::ProbeJlink) {
//     gdb_server.arg("-LocalHostOnly").arg("1");
//     gdb_server.arg("-Silent").arg("1");
//     gdb_server.arg("-Port").arg(config_probe_jlink.port.to_string());
//     gdb_server.arg("-NoReset").arg("1");
// }
//
// fn commander_script(commander: &mut Command, script: &Path) {
//     commander.arg("-AutoConnect").arg("1");
//     commander.arg("-ExitOnError").arg("1");
//     commander.arg("-CommandFile").arg(script);
// }
