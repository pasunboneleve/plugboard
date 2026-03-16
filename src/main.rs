fn main() {
    if let Err(error) = plugboard::cli::run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
