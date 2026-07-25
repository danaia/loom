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

SOURCE_HASH="$(sed -n 's/^source_graph_hash: //p' "${RESULT_FILE}")"
ARTIFACT_BEFORE="$(sed -n 's/^artifact_before_repair: //p' "${RESULT_FILE}")"
REPAIR_COUNT="$(sed -n 's/^repair_edits: //p' "${RESULT_FILE}")"
ARTIFACT_FINGERPRINT="$(sed -n 's/^artifact_fingerprint: //p' "${RESULT_FILE}")"

if [[ ! "${SOURCE_HASH}" =~ ^[0-9a-f]{64}$ ]]; then
  echo "Expected a 64-character normalized source-graph hash." >&2
  exit 1
fi

if [[ "${ARTIFACT_BEFORE}" != "none" ]]; then
  echo "Invalid graphs must not receive executable artifact fingerprints." >&2
  exit 1
fi

if [[ "${REPAIR_COUNT}" -ne 2 ]]; then
  echo "Expected one atomic plan containing two edits." >&2
  exit 1
fi

if [[ ! "${ARTIFACT_FINGERPRINT}" =~ ^[0-9a-f]{64}$ ]]; then
  echo "Expected a validated 64-character artifact fingerprint." >&2
  exit 1
fi

echo
echo "Loom validator-hardening milestone passed."
echo "Untrusted source graph: ${SOURCE_HASH}"
echo "Validated artifact:     ${ARTIFACT_FINGERPRINT}"
echo "Verified: invalid graphs receive no artifact identity; two edits applied atomically and revalidated."
