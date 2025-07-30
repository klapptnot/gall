use clap::{Args, Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "gall")]
#[command(about = "GTK based (apps) selector")]
#[command(version = concat!(
    env!("CARGO_PKG_VERSION"),
    " (",
    env!("GIT_HASH"),
    ")"
))]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Start the daemon with specified configuration
    Start(DaemonArgs),
    /// Stop the running daemon
    Stop,
    /// Toggle the app launcher visibility
    Apps,
    /// Reload daemon configuration
    Reload,
}

#[derive(Args)]
pub struct DaemonArgs {
    /// Path to the styles directory or CSS file
    #[arg(short, long, value_name = "PATH")]
    pub styles: Option<PathBuf>,

    /// Path to the configuration file
    #[arg(short, long, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Keep open here, without daemon behavior
    #[arg(long)]
    pub here: bool,
}
