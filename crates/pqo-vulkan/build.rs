use std::{env, path::PathBuf, process::Command};

fn main() {
    let output = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR"));
    for (source, stage, name) in [
        ("shaders/crystal.vert", "vert", "crystal.vert.spv"),
        ("shaders/crystal.frag", "frag", "crystal.frag.spv"),
        ("shaders/atom.frag", "frag", "atom.frag.spv"),
    ] {
        println!("cargo:rerun-if-changed={source}");
        let status = Command::new(env::var_os("PQO_GLSLC").unwrap_or_else(|| "glslc".into()))
            .args([
                "-O",
                "--target-env=vulkan1.3",
                &format!("-fshader-stage={stage}"),
                source,
                "-o",
            ])
            .arg(output.join(name))
            .status()
            .unwrap_or_else(|error| panic!("could not invoke glslc for {source}: {error}"));
        assert!(status.success(), "glslc failed for {source}");
    }
}
