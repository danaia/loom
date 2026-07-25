use loom_core::conformance::{HelloParticleConfig, hello_batch_builder, hello_particle_builder};
use loom_metal::MetalRuntime;
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
    let builder = match arguments.first().map(String::as_str) {
        None | Some("particle") => hello_particle_builder(HelloParticleConfig::default()),
        Some("batch") => {
            let count = arguments
                .get(1)
                .map(|value| parse_count(value))
                .transpose()?
                .unwrap_or(1_000);
            hello_batch_builder(count)
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
    MetalRuntime::run(validated)
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
    loom_metal::RuntimeDiagnostic {
        code: loom_metal::RuntimeDiagnosticCode::UnsupportedGraph,
        message: format!(
            "invalid particle count `{value}`; use a positive integer or suffix such as `1k` or `1m`"
        ),
        semantic_path: None,
    }
}

#[cfg(test)]
mod tests {
    use super::parse_count;

    #[test]
    fn parses_batch_count_suffixes() {
        assert_eq!(parse_count("1000").unwrap(), 1_000);
        assert_eq!(parse_count("10k").unwrap(), 10_000);
        assert_eq!(parse_count("1M").unwrap(), 1_000_000);
        assert!(parse_count("0").is_err());
    }
}
