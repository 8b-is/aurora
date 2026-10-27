//! `aurora` binary entry point.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match aurora_cli::run(&args) {
        Ok(output) => print!("{output}"),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
