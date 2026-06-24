//! `lk` — the link-toy CLI and daemon. Thin entry point; all logic lives in
//! [`linktoy::cli`], because "queries" are just CLI commands that read state.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(linktoy::cli::run(&args));
}
