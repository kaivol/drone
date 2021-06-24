#![warn(clippy::pedantic)]

use drone::{cli::Cli, utils::run_wrapper};
use structopt::StructOpt;

#[tokio::main]
async fn main() {
    let args = Cli::from_args();
    run_wrapper(args.color, args.run()).await;
}
