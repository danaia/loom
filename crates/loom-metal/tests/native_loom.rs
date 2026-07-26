#![cfg(target_os = "macos")]

use loom_metal::{BenchmarkConfig, BenchmarkMode, BenchmarkRunner, MetalRuntime};
use loom_syntax::parse;
use loom_validator::Validator;

const SOURCE: &str = include_str!("../../../examples/hello-particle/hello-particle.agent.loom");
const CRYSTAL_SOURCE: &str = include_str!("../../../examples/hello-crystal/crystal.loom");

#[test]
fn native_loom_kernel_generates_compiles_and_executes_metal() {
    let graph = parse(SOURCE).expect("agent-native source must parse");
    let integrate = graph
        .kernels
        .iter()
        .find(|kernel| kernel.name == "integrate")
        .expect("integration kernel");
    let implementation = &integrate.implementations[0];
    let generated = implementation
        .source_text
        .as_deref()
        .expect("native Loom kernel must package generated Metal");
    assert!(implementation.source.starts_with("loom://generated/"));
    assert!(generated.contains("kernel void integrate_main"));
    assert!(generated.contains("device packed_float3 *position [[buffer(0)]]"));

    let validated = Validator::validate(&graph)
        .validated
        .expect("generated graph must validate");
    let result = MetalRuntime::benchmark(
        validated,
        BenchmarkConfig {
            mode: BenchmarkMode::Headless,
            runner: BenchmarkRunner::LoomPlan,
            warmup_ticks: 0,
            sample_ticks: 1,
            ..BenchmarkConfig::default()
        },
    )
    .expect("generated Metal must compile and execute");

    assert_eq!(result.sample_ticks, 1);
    assert!(result.runtime.shader_hashes.iter().any(|shader| {
        shader.source_path == "loom://generated/hello_particle_agent/integrate.metal"
    }));
}

#[test]
fn crystal_loom_source_compiles_and_executes_packaged_metal() {
    let graph = parse(CRYSTAL_SOURCE).expect("crystal source must parse");
    let validated = Validator::validate(&graph)
        .validated
        .expect("crystal graph must validate");
    let result = MetalRuntime::benchmark(
        validated,
        BenchmarkConfig {
            mode: BenchmarkMode::Headless,
            runner: BenchmarkRunner::LoomPlan,
            warmup_ticks: 0,
            sample_ticks: 1,
            ..BenchmarkConfig::default()
        },
    )
    .expect("crystal Metal must compile and execute");

    assert_eq!(result.sample_ticks, 1);
    assert!(
        result
            .runtime
            .shader_hashes
            .iter()
            .any(|shader| shader.source_path == "kernels/crystal.metal")
    );
    assert!(
        result
            .runtime
            .shader_hashes
            .iter()
            .any(|shader| shader.source_path == "shaders/crystal.metal")
    );
}
