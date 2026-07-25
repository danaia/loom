#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
RESULT_FILE="$(mktemp "${TMPDIR:-/tmp}/loom-language-test.XXXXXX")"

cleanup() {
  rm -f "${RESULT_FILE}"
}
trap cleanup EXIT

cd "${REPO_ROOT}"

echo "==> Checking Rust formatting"
cargo fmt --all -- --check

echo "==> Running strict Clippy checks"
cargo clippy --workspace --all-targets -- -D warnings

echo "==> Running typed-graph and validator tests"
cargo test --workspace

echo "==> Exercising agent-actionable overlap diagnostics"
cargo run --quiet --package loom-validator --example hello_particle >"${RESULT_FILE}"

CONFLICT_COUNT="$(grep -c '"code": "InsufficientBufferVersions"' "${RESULT_FILE}")"
FIX_COUNT="$(grep -c '"SetStreamBuffering"' "${RESULT_FILE}")"

if [[ "${CONFLICT_COUNT}" -ne 2 ]]; then
  echo "Expected two insufficient-buffer diagnostics; found ${CONFLICT_COUNT}." >&2
  exit 1
fi

if [[ "${FIX_COUNT}" -ne 2 ]]; then
  echo "Expected two mechanical buffering fixes; found ${FIX_COUNT}." >&2
  exit 1
fi

FINGERPRINT="$(sed -n 's/^fingerprint: //p' "${RESULT_FILE}")"
if [[ ! "${FINGERPRINT}" =~ ^[0-9a-f]{64}$ ]]; then
  echo "Expected a 64-character SHA-256 graph fingerprint." >&2
  exit 1
fi

echo
echo "Loom language milestone passed."
echo "Graph fingerprint: ${FINGERPRINT}"
echo "Verified: unsafe one-buffer/four-tick overlap produced two GraphEdit repairs."
