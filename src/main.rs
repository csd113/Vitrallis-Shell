fn main() -> std::process::ExitCode {
    match vitrallis_shell::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("level=error event=startup message={error:?}");
            std::process::ExitCode::FAILURE
        }
    }
}
