//! `aurora` binary entry point.

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let output = aurora_cli::run(&args);
    print!("{output}");
}
