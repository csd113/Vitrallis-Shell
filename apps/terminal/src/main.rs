fn main() -> std::process::ExitCode {
    match vitrallis_terminal::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Terminal: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
