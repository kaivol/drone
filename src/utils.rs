//! Utility functions.
use std::{
    env,
    path::PathBuf,
    process::exit,
};
use std::future::Future;
use std::pin::Pin;
use std::task::{Context, Poll};

use ansi_term::Color::Red;
use anyhow::{bail, Result, anyhow};
use futures::Stream;
use futures::prelude::*;
use futures::StreamExt;
use pin_project::pin_project;
use serde::{de, ser};
use thiserror::Error;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::process::{Child, Command};
use walkdir::WalkDir;

use crate::color::Color;

/// Runs the application code inside closure `f`, prints an error using `color`
/// preference if there is any, and sets the exit code.
pub async fn run_wrapper(
    color: Color,
    f: impl Future<Output=Result<()>>,
) {
    match f.await {
        Ok(()) => {
            exit(0);
        }
        Err(err) if err.is::<SignalError>() => {
            exit(1);
        }
        Err(err) => {
            eprintln!("{}: {:?}", color.bold_fg("Error", Red), err);
            exit(1);
        }
    }
}

/// Returns the current crate root.
pub async fn crate_root() -> Result<PathBuf> {
    let mut cargo = Command::new("cargo");
    cargo.arg("locate-project").arg("--message-format=plain");
    let mut root = PathBuf::from(String::from_utf8(cargo.output().await?.stdout)?);
    root.pop();
    Ok(root)
}

/// Searches for the Rust tool `tool` in the sysroot.
pub async fn search_rust_tool(tool: &str) -> Result<PathBuf> {
    let mut rustc = Command::new("rustc");
    rustc.arg("--print").arg("sysroot");
    let sysroot = String::from_utf8(rustc.output().await?.stdout)?;
    for entry in WalkDir::new(sysroot.trim()) {
        let entry = entry?;
        if matches!(entry.path().file_stem(), Some(stem) if stem == tool) {
            return Ok(entry.into_path());
        }
    }
    bail!("Couldn't find `{}`", tool);
}

/// Runs the command and checks its exit status.
pub async fn run_command(mut command: Command) -> Result<()> {
    match command.status().await {
        Ok(status) if status.success() => Ok(()),
        Ok(status) => {
            if let Some(code) = status.code() {
                bail!("`{:?}` exited with status code: {}", command, code)
            }
            bail!("`{:?}` terminated by signal", command,)
        }
        Err(err) => bail!("`{:?}` failed to execute: {}", command, err),
    }
}

/// Spawns the command and checks for errors.
pub fn spawn_command(mut command: Command) -> Result<Child> {
    match command.spawn() {
        Ok(child) => Ok(child),
        Err(err) => bail!("`{:?}` failed to execute: {}", command, err),
    }
}

/// Possible encountered signals
#[derive(Eq, PartialEq, Copy, Clone, Debug)]
pub enum Signals {
    SigInt,
    #[cfg(unix)] SigQuit,
    #[cfg(unix)] SigTerm,
    #[cfg(windows)] SigBreak,
}

pub type SignalStream = impl Stream<Item=Signals> + Unpin;

/// Register desired signals.
#[cfg(unix)]
pub fn register_signals() -> Result<SignalStream> {
    let mut interrupt = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;
    let mut quit = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::quit())?;
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;

    Ok(stream::select(
        stream::poll_fn(move |cx| {
            interrupt.poll_recv(cx).map(|o| o.map(|_| Signals::SigInt))
        }),
        stream::select(
            stream::poll_fn(move |cx| {
                quit.poll_recv(cx).map(|o| o.map(|_| Signals::SigQuit))
            }),
            stream::poll_fn(move |cx| {
                terminate.poll_recv(cx).map(|o| o.map(|_| Signals::SigTerm))
            }),
        )
    ))
}

/// Register desired signals.
#[cfg(windows)]
pub fn register_signals() -> Result<SignalStream> {
    let mut ctrl_c = tokio::signal::windows::ctrl_c()?;
    let mut ctrl_break = tokio::signal::windows::ctrl_break()?;
    Ok(stream::select(
        stream::poll_fn(move |cx| {
            ctrl_c.poll_recv(cx).map(|o| o.map(|_| Signals::SigInt))
        }),
        stream::poll_fn(move |cx| {
            ctrl_break.poll_recv(cx).map(|o| o.map(|_| Signals::SigBreak))
        }),
    ))
}


pub trait WithSignals<T, E, Fut> where
    Fut: Send + Future<Output=core::result::Result<T, E>>,
    E: Into<anyhow::Error> + Send + Sync + 'static,
{
    fn with_signals(self, signals: &mut SignalStream, ignore_sigint: bool) -> SignalFuture<T, E, Fut>;
}

impl<T, E, Fut> WithSignals<T, E, Fut> for Fut where
    Fut: Send + Future<Output=core::result::Result<T, E>>,
    E: Into<anyhow::Error> + Send + Sync + 'static,
{
    fn with_signals(self, signals: &mut SignalStream, ignore_sigint: bool) -> SignalFuture<T, E, Fut> {
        SignalFuture {
            future:self,
            signals,
            ignore_sigint
        }
    }
}

#[pin_project]
pub struct SignalFuture<'a, T, E, Fut> where
    Fut: Send + Future<Output=core::result::Result<T, E>>,
    E: Into<anyhow::Error> + Send + Sync + 'static,
{
    #[pin]
    future: Fut,
    signals: &'a mut SignalStream,
    ignore_sigint: bool
}

impl<'a, T, E, Fut> Future for SignalFuture<'a, T, E, Fut> where
    Fut: Send + Future<Output=core::result::Result<T, E>>,
    E: Into<anyhow::Error> + Send + Sync + 'static,
{
    type Output = anyhow::Result<T>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        match this.future.poll(cx) {
            Poll::Ready(result) => Poll::Ready(match result {
                Ok(val) => Ok(val),
                Err(error) => Err(error.into()),
            }),
            Poll::Pending => {
                match this.signals.poll_next_unpin(cx) {
                    Poll::Ready(val) => {
                        if let Some(signal) = val {
                            if signal == Signals::SigInt && *this.ignore_sigint {
                                Poll::Pending
                            } else {
                                Poll::Ready(Err(SignalError.into()))
                            }
                        } else {
                            unreachable!()
                        }
                    }
                    Poll::Pending => Poll::Pending
                }
            }
        }
    }
}

// impl<T, Fut> WithSignals<T> for Fut where
//     Fut: Send + Future<Output=anyhow::Result<T>>,
// {
//     async fn with_signals(self, signals: &mut SignalStream, ignore_sigint: bool) -> Result<T> {
//         let future = self;
//         tokio::pin!(future);
//         loop {
//             tokio::select! {
//                 result = &mut future => return result,
//                 signal = signals.next() => {
//                     if let Some(signal) = signal {
//                         if signal != Signals::SigInt || !ignore_sigint {
//                             bail!(SignalError);
//                         }
//                     }
//                 },
//             }
//         }
//     }
// }
//
// /// Run the closure in a different thread, periodically checking the signals.
// pub async fn block_with_signals<T, E, Fut>(
//     signals: &mut SignalStream,
//     ignore_sigint: bool,
//     future: Fut,
// ) -> Result<T> where
//     Fut: Send + Future<Output=core::result::Result<T, E>>,
//     E: Into<anyhow::Error> + Send + Sync + 'static,
// {
//     tokio::pin!(future);
//     loop {
//         tokio::select! {
//             result = &mut future => return match result {
//                 Ok(t) => Ok(t),
//                 Err(e) => Err(e.into())
//             },
//             signal = signals.next() => {
//                 if let Some(signal) = signal {
//                     if signal != Signals::SigInt || !ignore_sigint {
//                         bail!(SignalError);
//                     }
//                 }
//             },
//         }
//     }
// }

/// Run the closure in a different thread, periodically checking the signals.
// pub async fn block_with_signals<T, Fut>(
//     signals: &mut SignalStream,
//     ignore_sigint: bool,
//     future: Fut,
// ) -> Result<T> where
//     Fut: Send + Future<Output=anyhow::Result<T>>,
// {
//     tokio::pin!(future);
//     loop {
//         tokio::select! {
//             result = &mut future => return match result {
//                 Ok(t) => Ok(t),
//                 Err(e) => Err(e.into())
//             },
//             signal = signals.next() => {
//                 if let Some(signal) = signal {
//                     if signal != Signals::SigInt || !ignore_sigint {
//                         bail!(SignalError);
//                     }
//                 }
//             },
//         }
//     }
// }

// /// Runs the closure when the returned object is dropped.
// pub fn finally<F: FnOnce()>(f: F) -> impl Drop {
//     struct Finalizer<F: FnOnce()>(Option<F>);
//     impl<F: FnOnce()> Drop for Finalizer<F> {
//         fn drop(&mut self) {
//             self.0.take().unwrap()();
//         }
//     }
//     Finalizer(Some(f))
// }

/// Returns the directory for temporary files.
pub fn temp_dir() -> PathBuf {
    env::var_os("XDG_RUNTIME_DIR").map_or(env::temp_dir(), Into::into)
}

// /// Creates a new fifo.
// pub fn make_fifo(dir: &TempDir, name: &str) -> Result<PathBuf> {
//     let pipe = dir.path().join(name);
//     let c_pipe = CString::new(pipe.as_os_str().as_bytes())?;
//     if unsafe { libc::mkfifo(c_pipe.as_ptr(), 0o644) } == -1 {
//         return Err(std::io::Error::last_os_error().into());
//     }
//     Ok(pipe)
// }

// /// Consumes all remaining data in the fifo.
// pub fn exhaust_fifo(path: &str) -> Result<()> {
//     let mut fifo = OpenOptions::new().read(true).open(path)?;
//     unsafe { libc::fcntl(fifo.as_raw_fd(), libc::F_SETFL, libc::O_NONBLOCK) };
//     let mut bytes = [0_u8; 1024];
//     loop {
//         match fifo.read(&mut bytes) {
//             Ok(_) => continue,
//             Err(ref err) if err.kind() == ErrorKind::Interrupted => continue,
//             Err(ref err) if err.kind() == ErrorKind::WouldBlock => break Ok(()),
//             Err(err) => break Err(err.into()),
//         }
//     }
// }

// /// Moves the process to a new process group.
// pub fn detach_pgid(command: &mut Command) {
//     unsafe {
//         command.pre_exec(|| {
//             libc::setpgid(0, 0);
//             Ok(())
//         });
//     }
// }

#[non_exhaustive]
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
struct CargoConfig {
    build: Option<CargoConfigBuild>,
}

#[non_exhaustive]
#[derive(Clone, Debug, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
struct CargoConfigBuild {
    target: Option<String>,
}

/// Resolve target for current crate
pub async fn resolve_target() -> Result<String>{
    let crate_root = crate_root().await?.canonicalize()?;
    let path = crate_root.join(".cargo").join("config");
    if !path.exists() {
        bail!("`{}` does not exist in `{}", path.display(), crate_root.display());
    }
    let mut buffer = String::new();
    let mut file = File::open(&path).await?;
    file.read_to_string(&mut buffer).await?;
    let config = toml::from_str::<CargoConfig>(&buffer)?;
    let target = config
        .build
        .and_then(|build| build.target)
        .ok_or_else(|| anyhow!("No [build.target] configuration in {:?}", path))?;
    Ok(target)
}

/// Serialize the value to a string.
pub fn ser_to_string<T: ser::Serialize>(value: T) -> String {
    serde_json::to_value(value).unwrap().as_str().unwrap().to_string()
}

/// Deserialize a value from the string.
pub fn de_from_str<T: de::DeserializeOwned>(s: &str) -> Result<T> {
    serde_json::from_value(serde_json::Value::String(s.to_string())).map_err(Into::into)
}

#[derive(Error, Debug)]
#[error("signal")]
struct SignalError;
