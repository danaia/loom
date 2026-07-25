use loom_core::conformance::{HelloParticleConfig, hello_batch_builder, hello_particle_builder};
use loom_metal::{BenchmarkConfig, BenchmarkMode, MetalRuntime};
use loom_validator::Validator;

fn main() {
    if let Err(diagnostic) = run() {
        eprintln!(
            "{}",
            serde_json::to_string_pretty(&diagnostic).unwrap_or_else(|_| diagnostic.to_string())
        );
        std::process::exit(1);
    }
}

fn run() -> Result<(), loom_metal::RuntimeDiagnostic> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    let (builder, benchmark) = match arguments.first().map(String::as_str) {
        None | Some("particle") => (hello_particle_builder(HelloParticleConfig::default()), None),
        Some("batch") => {
            let (count, benchmark) = parse_batch_arguments(&arguments[1..])?;
            (hello_batch_builder(count), benchmark)
        }
        Some(other) => {
            return Err(loom_metal::RuntimeDiagnostic {
                code: loom_metal::RuntimeDiagnosticCode::UnsupportedGraph,
                message: format!(
                    "unknown experiment `{other}`; expected `particle` or `batch [COUNT]`"
                ),
                semantic_path: None,
            });
        }
    };
    let graph = builder
        .build()
        .map_err(|diagnostics| loom_metal::RuntimeDiagnostic {
            code: loom_metal::RuntimeDiagnosticCode::UnsupportedGraph,
            message: serde_json::to_string(&diagnostics)
                .unwrap_or_else(|_| "Hello Particle graph build failed".to_owned()),
            semantic_path: None,
        })?;
    let report = Validator::validate(&graph);
    let validated = report
        .validated
        .ok_or_else(|| loom_metal::RuntimeDiagnostic {
            code: loom_metal::RuntimeDiagnosticCode::UnsupportedGraph,
            message: serde_json::to_string(&report.diagnostics)
                .unwrap_or_else(|_| "Hello Particle validation failed".to_owned()),
            semantic_path: None,
        })?;
    if let Some(config) = benchmark {
        let result = MetalRuntime::benchmark(validated, config)?;
        println!(
            "{}",
            serde_json::to_string_pretty(&result).expect("benchmark result serialization")
        );
        Ok(())
    } else {
        MetalRuntime::run(validated)
    }
}

fn parse_batch_arguments(
    arguments: &[String],
) -> Result<(u32, Option<BenchmarkConfig>), loom_metal::RuntimeDiagnostic> {
    let mut index = 0;
    let count = if arguments
        .first()
        .is_some_and(|argument| !argument.starts_with("--"))
    {
        index += 1;
        parse_count(&arguments[0])?
    } else {
        1_000
    };
    let mut benchmark: Option<BenchmarkConfig> = None;
    while index < arguments.len() {
        match arguments[index].as_str() {
            "--bench" => {
                let mut config = benchmark.take().unwrap_or_default();
                if let Some(mode) = arguments.get(index + 1).map(String::as_str) {
                    match mode {
                        "headless" => {
                            config.mode = BenchmarkMode::Headless;
                            index += 1;
                        }
                        "rendered" => {
                            config.mode = BenchmarkMode::Rendered;
                            index += 1;
                        }
                        _ => {}
                    }
                }
                benchmark = Some(config);
            }
            "--warmup" => {
                let value = parse_positive_option(arguments, index, "--warmup")?;
                benchmark
                    .get_or_insert_with(BenchmarkConfig::default)
                    .warmup_ticks = value;
                index += 1;
            }
            "--samples" => {
                let value = parse_positive_option(arguments, index, "--samples")?;
                benchmark
                    .get_or_insert_with(BenchmarkConfig::default)
                    .sample_ticks = value;
                index += 1;
            }
            other => {
                return Err(argument_error(format!(
                    "unknown batch option `{other}`; use `--bench [headless|rendered]`, `--warmup N`, or `--samples N`"
                )));
            }
        }
        index += 1;
    }
    Ok((count, benchmark))
}

fn parse_positive_option(
    arguments: &[String],
    index: usize,
    option: &str,
) -> Result<u32, loom_metal::RuntimeDiagnostic> {
    arguments
        .get(index + 1)
        .and_then(|value| value.parse::<u32>().ok())
        .filter(|value| *value > 0)
        .ok_or_else(|| argument_error(format!("{option} requires a positive integer")))
}

fn parse_count(value: &str) -> Result<u32, loom_metal::RuntimeDiagnostic> {
    let normalized = value.trim().to_ascii_lowercase();
    let (digits, multiplier) = match normalized.as_bytes().last() {
        Some(b'k') => (&normalized[..normalized.len() - 1], 1_000_u32),
        Some(b'm') => (&normalized[..normalized.len() - 1], 1_000_000_u32),
        _ => (normalized.as_str(), 1_u32),
    };
    let base = digits.parse::<u32>().map_err(|_| invalid_count(value))?;
    let count = base
        .checked_mul(multiplier)
        .filter(|count| *count > 0)
        .ok_or_else(|| invalid_count(value))?;
    Ok(count)
}

fn invalid_count(value: &str) -> loom_metal::RuntimeDiagnostic {
    argument_error(format!(
        "invalid particle count `{value}`; use a positive integer or suffix such as `1k` or `1m`"
    ))
}

fn argument_error(message: String) -> loom_metal::RuntimeDiagnostic {
    loom_metal::RuntimeDiagnostic {
        code: loom_metal::RuntimeDiagnosticCode::UnsupportedGraph,
        message,
        semantic_path: None,
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_batch_arguments, parse_count};
    use loom_metal::BenchmarkMode;

    #[test]
    fn parses_batch_count_suffixes() {
        assert_eq!(parse_count("1000").unwrap(), 1_000);
        assert_eq!(parse_count("10k").unwrap(), 10_000);
        assert_eq!(parse_count("1M").unwrap(), 1_000_000);
        assert!(parse_count("0").is_err());
    }

    #[test]
    fn parses_benchmark_options() {
        let arguments = [
            "100k",
            "--bench",
            "rendered",
            "--warmup",
            "10",
            "--samples",
            "20",
        ]
        .map(str::to_owned);
        let (count, benchmark) = parse_batch_arguments(&arguments).unwrap();
        let benchmark = benchmark.unwrap();
        assert_eq!(count, 100_000);
        assert_eq!(benchmark.mode, BenchmarkMode::Rendered);
        assert_eq!(benchmark.warmup_ticks, 10);
        assert_eq!(benchmark.sample_ticks, 20);
    }
}
