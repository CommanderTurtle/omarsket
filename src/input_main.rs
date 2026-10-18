fn main() {
    #[cfg(target_os = "linux")]
    let result = omarsket::remap::run();
    #[cfg(not(target_os = "linux"))]
    let result: Result<(), String> = Err("Input routing requires Linux".into());
    if let Err(error) = result {
        #[cfg(target_os = "linux")]
        omarsket::remap::report_error(&error);
        eprintln!("{error}");
        std::process::exit(1);
    }
}
