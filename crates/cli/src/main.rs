use clap::Parser;

fn main() {
    if let Err(e) = crablink_cli::run(crablink_cli::Cli::parse()) {
        // The reader went away (`| head`): nothing left to say, and nobody to say it to.
        let broken_pipe = e
            .root_cause()
            .downcast_ref::<std::io::Error>()
            .is_some_and(|e| e.kind() == std::io::ErrorKind::BrokenPipe);
        if broken_pipe {
            return;
        }
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}
