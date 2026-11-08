//! `pakscope` command line tool.

use clap::Parser;

/// Inspect and extract resources from game archives.
#[derive(Parser)]
#[command(name = "pakscope", version, about)]
struct Cli {}

fn main() {
    let _cli = Cli::parse();

    println!(
        "pakscope {} (core {})",
        env!("CARGO_PKG_VERSION"),
        pakscope_core::version()
    );
}
