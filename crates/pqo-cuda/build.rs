fn main() {
    if std::env::var_os("CARGO_CFG_TARGET_OS").as_deref() == Some(std::ffi::OsStr::new("linux")) {
        cc::Build::new()
            .file("src/interop_probe.c")
            .include("/usr/local/cuda/include")
            .compile("pqo_cuda_interop_probe");
        println!("cargo:rustc-link-lib=cuda");
        println!("cargo:rerun-if-changed=src/interop_probe.c");
    }
}
