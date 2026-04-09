fn main() {
    if let Err(err) = melo::cli::run() {
        eprintln!("{err}");
        std::process::exit(1);
    }
}
