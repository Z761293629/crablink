use clap::Parser;

fn main() {
    if let Err(e) = crablink_cli::run(crablink_cli::Cli::parse()) {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}
