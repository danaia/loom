#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

WARMUP_SECONDS="${1:-5}"
SAMPLE_SECONDS="${2:-10}"
OUTPUT_DIR="${3:-${REPO_ROOT}/benchmark-results/hello-batch}"

if ! [[ "${WARMUP_SECONDS}" =~ ^[1-9][0-9]*$ ]]; then
  echo "warm-up seconds must be a positive integer" >&2
  exit 2
fi
if ! [[ "${SAMPLE_SECONDS}" =~ ^[1-9][0-9]*$ ]]; then
  echo "sample seconds must be a positive integer" >&2
  exit 2
fi

mkdir -p "${OUTPUT_DIR}"

for count in 1k 10k 100k 1m; do
  for mode in headless rendered; do
    output="${OUTPUT_DIR}/${count}-${mode}.json"
    echo "==> Hello Batch ${count} ${mode}"
    "${SCRIPT_DIR}/run-hello-particle.sh" \
      batch "${count}" \
      --bench "${mode}" \
      --warmup-seconds "${WARMUP_SECONDS}" \
      --duration-seconds "${SAMPLE_SECONDS}" \
      >"${output}"
    echo "    ${output}"
  done
done
