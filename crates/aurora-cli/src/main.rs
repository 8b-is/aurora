//! `aurora` binary entry point.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match aurora_cli::run(&args) {
        Ok(output) => {
            use std::io::{self, Write};
            let mut stdout = io::stdout().lock();
            if let Err(e) = write!(stdout, "{}", output) {
                eprintln!("failed to write to stdout: {e}");
                std::process::exit(1);
            }
            let _ = stdout.flush();
        }
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
