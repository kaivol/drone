//! OpenOCD.

use super::{
    gdb_script_command,
    run_gdb_server, rustc_substitute_path,
};
use crate::{
    cli::{FlashCmd, GdbCmd, LogCmd, ResetCmd},
    color::Color,
    log,
    templates::Registry,
    utils::{run_command, spawn_command},
};
use anyhow::Result;
use drone_config as config;
use tokio::process::{Command, Child};
use crate::utils::{SignalStream, WithSignals};
use std::path::PathBuf;
use std::io::{Read, stderr};
use tokio::io::{AsyncRead, ReadBuf, Error, ErrorKind, AsyncWriteExt};
use std::net::{Ipv4Addr, SocketAddr, IpAddr};
use tokio::net::{TcpListener, TcpSocket};
use tokio::sync::mpsc;
use std::task::{Context, Poll};
use std::pin::Pin;
use crate::probe::run_gdb_client;
use crate::log::OutputMap;
use drone_config::ProbeOpenocd;
use std::str::FromStr;
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::process::Stdio;
use tokio::time::sleep;
use tokio::time::Duration;
use futures::StreamExt;

// /// Runs `drone reset` command.
// pub async fn reset(
//     cmd: ResetCmd,
//     mut signals: SignalStream,
//     registry: Registry<'_>,
//     config: config::Config,
// ) -> Result<()> {
//     let ResetCmd {} = cmd;
//     let config_probe_openocd = config.probe.as_ref().unwrap().openocd.as_ref().unwrap();
//     let commands = registry.openocd_reset()?;
//     let mut openocd = Command::new(&config_probe_openocd.command);
//     openocd_arguments(&mut openocd, config_probe_openocd);
//     openocd_commands(&mut openocd, &commands);
//     run_command(openocd).with_signals(&mut signals, true).await?;
//     Ok(())
// }

// /// Runs `drone flash` command.
// pub async fn flash(
//     cmd: FlashCmd,
//     mut signals: SignalStream,
//     registry: Registry<'_>,
//     config: config::Config,
// ) -> Result<()> {
//     let FlashCmd { firmware } = cmd;
//     let config_probe_openocd = config.probe.as_ref().unwrap().openocd.as_ref().unwrap();
//     let commands = registry.openocd_flash(&firmware)?;
//     let mut openocd = Command::new(&config_probe_openocd.command);
//     openocd_arguments(&mut openocd, config_probe_openocd);
//     openocd_commands(&mut openocd, &commands);
//     run_command(openocd).with_signals(&mut signals, true).await?;
//     Ok(())
// }

// /// Runs `drone gdb` command.
// pub async fn gdb(
//     cmd: GdbCmd,
//     mut signals: SignalStream,
//     registry: Registry<'_>,
//     config: config::Config,
// ) -> Result<()> {
//     let GdbCmd { firmware, reset, interpreter, gdb_args } = cmd;
//     let config_probe_openocd = config.probe.as_ref().unwrap().openocd.as_ref().unwrap();
//
//     let commands = registry.openocd_gdb_openocd(&config)?;
//     let mut openocd = Command::new(&config_probe_openocd.command);
//     openocd_arguments(&mut openocd, config_probe_openocd);
//     openocd_commands(&mut openocd, &commands);
//     run_gdb_server(
//         openocd,
//         interpreter.as_ref().map(String::as_ref),
//         async {
//             let script = registry.openocd_gdb_gdb(&config, reset, &rustc_substitute_path().await?)?;
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

struct OpenocdCommand {
    command: Command,
}

impl OpenocdCommand {
    fn new(config_probe_openocd: &ProbeOpenocd) -> OpenocdCommand {
        let mut command = Command::new(&(config_probe_openocd.command));
        for argument in config_probe_openocd.arguments.iter() {
            command.arg(argument);
        }
        // command.stdout(Stdio::null());
        // command.stderr(Stdio::null());
        command.kill_on_drop(true);
        OpenocdCommand {
            command,
        }
    }

    fn add_command(&mut self, command: impl AsRef<OsStr>){
        self.command.arg("-c").arg(command);
    }

    fn spawn(&mut self) -> futures::io::Result<Child> {
        self.command.spawn()
    }
}

/// Runs `drone log` command.
pub async fn log_swo(
    cmd: LogCmd,
    mut signals: SignalStream,
    _: Registry<'_>,
    config: config::Config,
    color: Color,
) -> Result<()> {
    let LogCmd { reset, outputs } = cmd;
    let config_probe_openocd = config.probe.as_ref().unwrap().openocd.as_ref().unwrap();
    let config_log_swo = config.log.as_ref().unwrap().swo.as_ref().unwrap();

    let mut openocd = OpenocdCommand::new(&config_probe_openocd);
    // openocd.add_command("gdb_port disabled");
    // openocd.add_command("tcl_port disabled");
    // openocd.add_command("telnet_port disabled");
    openocd.add_command("init");
    // sleep(Duration::from_micros(10_000)).await;

    // let mut stream = TcpSocket::new_v4()?.connect(
    //     SocketAddr::from_str("127.0.0.1:4444")?
    // ).await?;
    if reset {
        openocd.add_command("reset halt");
        // stream.write(b"reset halt").await?;
    }

    // let ports: BTreeSet<u32> = outputs.iter().flat_map(|output| output.ports.iter().copied()).collect();
    // if !(ports.contains(&0u32)) {
    //     openocd.add_command("itm port 0 off");
    //     // stream.write(b"itm port 0 off").await?;
    // }
    // for port in ports.iter() {
    //     openocd.add_command(format!("itm port {} on", port));
    //     // stream.write(format!("itm port {} on", port).as_bytes()).await?;
    // }
    openocd.add_command(format!("itm ports on"));

    if let Some(serial_endpoint) = &config_log_swo.serial_endpoint {
        // use SWO over serial
        openocd.add_command(format!(
            "tpiu config external uart off {} {}",
            config_log_swo.reset_freq,
            config_log_swo.baud_rate,
        ));

        log::capture(serial_endpoint, config_log_swo.baud_rate,/*input, */OutputMap::new(&outputs)?, log::swo::parser, color);

        let mut openocd = openocd.spawn()?;

        let exit_code = openocd.wait().with_signals(&mut signals, true).await?;

        // let input = serial_input(serial_endpoint, config_log_swo.baud_rate).await?;
        // stream.write(format!(
        //     "tpiu config external uart off {} {}",
        //     config_log_swo.reset_freq,
        //     config_log_swo.baud_rate,
        // ).as_bytes()).await?;
        // let script = registry.openocd_swo_gdb(&config, &ports, reset, None)?;
        // spawn_command(gdb_script_command(&config, None, script.path()))?
    } else {
        panic!("unsupported");
        // SWO via openocd
        // const PORT: u16 = 7800;

        // let script = registry.openocd_swo_gdb(&config, &ports, reset, Some(PORT))?;
        // let gdb = spawn_command(gdb_script_command(&config, None, script.path()))?;
        //
        // let listener = TcpListener::bind((Ipv4Addr::new(127, 0, 0, 1), PORT)).await?;
        // let input = listener.accept().await?.0;
        // log::capture(input, OutputMap::new(&outputs)?, log::swo::parser, color).await;
        //
        // gdb
    };

    Ok(())
}

async fn serial_input(serial_endpoint: &str, baud_rate: u32) -> Result<impl AsyncRead + Send>{
    let (sender, receiver) = mpsc::channel(512);
    let serial = mio_serial::new(serial_endpoint, baud_rate).open()?;
    tokio::spawn(async move {
        for byte in serial.bytes() {
            sender.send(byte.unwrap()).await.unwrap();
        }
    });
    Ok(AsyncReadReceiver(receiver))
}

struct AsyncReadReceiver(mpsc::Receiver<u8>);

impl AsyncRead for AsyncReadReceiver{
    fn poll_read(self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<std::io::Result<()>> {
        self.get_mut().0.poll_recv(cx).map(|x| match x {
            None => Err(Error::from(ErrorKind::Other)),
            Some(byte) => {
                buf.put_slice(&[byte]);
                Ok(())
            }
        })
    }
}
