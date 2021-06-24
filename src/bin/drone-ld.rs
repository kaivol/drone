#![warn(clippy::pedantic)]

use anyhow::Result;
use drone::{
    color::Color,
    templates::Registry,
    utils::{
        crate_root, register_signals, run_command, run_wrapper,
        search_rust_tool,
    },
};
use drone_config::Config;
use std::{
    collections::HashMap,
    env,
    ffi::{OsStr, OsString},
    fs::{create_dir_all},
    path::Path,
};
use tokio::process::Command;
use drone::utils::WithSignals;
use tokio::time::Duration;
use tokio::fs::File;
use tokio::io::{BufReader, AsyncBufReadExt};
use futures::StreamExt;

#[tokio::main]
async fn main() {
    run_wrapper(Color::Never, run()).await;
}

async fn run() -> Result<()> {
    let args = env::args_os().skip(1).collect::<Vec<_>>();
    let config = Config::read_from_current_dir()?;
    let registry = Registry::new()?;
    let mut signals = register_signals()?;

    let crate_root = crate_root().await?;
    let target = drone::utils::resolve_target().await?;
    let target = crate_root.join("target").join(target);
    create_dir_all(&target)?;
    let stage_one = target.join("layout.ld.1");
    let stage_two = target.join("layout.ld.2");
    {
        let stage_one_file = std::fs::File::create(&stage_one)?;
        let stage_two_file = std::fs::File::create(&stage_two)?;
        registry.layout_ld(&config, false, &stage_one_file)?;
        registry.layout_ld(&config, true, &stage_two_file)?;
    }

    let linker = linker_command(stage_one.as_ref(), &args, &[]).await?;
    run_command(linker).with_signals(&mut signals, true).await?;

    let output_file = output(&args).await?;
    let size = size_command(&output_file).await?;
    let syms = run_size(size)
        .with_signals(&mut signals, true).await?
        .into_iter()
        .map(|(name, size)| format!("--defsym=_{}_section_size={}", name, size))
        .collect::<Vec<_>>();

    let linker = linker_command(stage_two.as_ref(), &args, &syms).await?;
    run_command(linker).with_signals(&mut signals, true).await?;

    Ok(())
}

async fn output(args: &Vec<OsString>) -> Result<OsString> {
    let position = args.iter().position(|arg| arg == "-o");
    if let Some(position) = position {
        return Ok(args[position + 1].clone());
    }

    let files = args.iter().filter_map(|x|
        if let Some(x) = x.to_str() {
            if x.starts_with('@') {
                Some(x[1..].to_owned())
            } else {
                None
            }
        } else {
            panic!("Invalid string")
        }
    );
    for file in files {
        let file = File::open(file).await?;
        let mut reader = BufReader::new(file);
        let mut lines = reader.lines();
        while let Some(line) = lines.next_line().await? {
            if line == "-o" {
                return Ok(lines.next_line().await?.unwrap().into())
            }
        }
    }
    panic!("Could not determine output")
}

async fn linker_command(script: &Path, args: &[OsString], syms: &[String]) -> Result<Command> {
    let mut rust_lld = Command::new(search_rust_tool("rust-lld").await?);
    rust_lld.arg("-flavor").arg("gnu");
    rust_lld.arg("-T").arg(script);
    rust_lld.args(args);
    rust_lld.args(syms);
    Ok(rust_lld)
}

async fn size_command(output: &OsStr) -> Result<Command> {
    let mut command = Command::new(search_rust_tool("llvm-size").await?);
    command.arg("-A").arg(output);
    Ok(command)
}

async fn run_size(mut command: Command) -> Result<HashMap<String, u32>> {
    let stdout = String::from_utf8(command.output().await?.stdout)?;
    let mut map = HashMap::new();
    for line in stdout.lines() {
        if line.starts_with('.') {
            if let [name, size, ..] = line.split_whitespace().collect::<Vec<_>>().as_slice() {
                map.insert(name[1..].to_string(), size.parse()?);
            }
        }
    }
    Ok(map)
}
