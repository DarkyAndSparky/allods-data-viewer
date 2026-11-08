//! `adv` command line tool of Allods Data Viewer.

use clap::Parser;

/// Inspect and extract resources from game archives.
#[derive(Parser)]
#[command(name = "adv", version, about)]
struct Cli {}

fn main() {
    let _cli = Cli::parse();

    println!(
        "Allods Data Viewer {} (core {})",
        env!("CARGO_PKG_VERSION"),
        adv_core::version()
    );
}
