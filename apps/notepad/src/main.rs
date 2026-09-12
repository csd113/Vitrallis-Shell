fn main() -> std::process::ExitCode {
    match vitrallis_notepad::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Notepad: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
