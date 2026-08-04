fn main() {
    if let Err(error) = pqo_vulkan::run_native_window(Default::default()) {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
