fn main() -> std::process::ExitCode {
    match vitrallis_files::run() {
        Ok(()) => std::process::ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Files: {error}");
            std::process::ExitCode::FAILURE
        }
    }
}
