fn main() {
    if let Err(error) = plugboard::cli::run() {
        if let plugboard::error::PlugboardError::SilentExit { code } = error {
            std::process::exit(code);
        }
        eprintln!("{error}");
        std::process::exit(1);
    }
}
