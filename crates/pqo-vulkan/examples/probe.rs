fn main() {
    match pqo_vulkan::probe_external_resources(0, 4096) {
        Ok(report) => println!(
            "{}",
            serde_json::to_string_pretty(&report).expect("serializable report")
        ),
        Err(error) => {
            eprintln!("{error}");
            std::process::exit(1);
        }
    }
}
