#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

COUNT="${1:-100k}"
MODE="${2:-rendered}"
WARMUP_SECONDS="${3:-30}"
SAMPLE_SECONDS="${4:-60}"
TRIALS="${5:-4}"
OUTPUT_DIR="${6:-${REPO_ROOT}/benchmark-results/hello-batch-clean}"

case "${MODE}" in
  headless|rendered|presented) ;;
  *)
    echo "mode must be headless, rendered, or presented" >&2
    exit 2
    ;;
esac
for value in "${WARMUP_SECONDS}" "${SAMPLE_SECONDS}" "${TRIALS}"; do
  if ! [[ "${value}" =~ ^[1-9][0-9]*$ ]]; then
    echo "durations and trial count must be positive integers" >&2
    exit 2
  fi
done

cd "${REPO_ROOT}"
if [[ -n "$(git status --porcelain)" ]]; then
  echo "refusing to benchmark a dirty tree; commit or stash changes first" >&2
  exit 2
fi

SOURCE_REVISION="$(git rev-parse HEAD)"
mkdir -p "${OUTPUT_DIR}"
printf '%s\n' "${SOURCE_REVISION}" >"${OUTPUT_DIR}/source-revision.txt"

run_trial() {
  local trial="$1"
  local runner="$2"
  local output="${OUTPUT_DIR}/trial-${trial}-${runner}.json"
  echo "==> Trial ${trial}/${TRIALS}: ${runner}"
  "${SCRIPT_DIR}/run-hello-particle.sh" \
    batch "${COUNT}" \
    --bench "${MODE}" \
    --runner "${runner}" \
    --pace 120 \
    --warmup-seconds "${WARMUP_SECONDS}" \
    --duration-seconds "${SAMPLE_SECONDS}" \
    >"${output}"
  echo "    ${output}"
}

for ((trial = 1; trial <= TRIALS; trial += 1)); do
  if ((trial % 2 == 1)); then
    run_trial "${trial}" pqo
    run_trial "${trial}" direct-metal
  else
    run_trial "${trial}" direct-metal
    run_trial "${trial}" pqo
  fi
done

echo "Clean-tree interleaved comparison complete at ${SOURCE_REVISION}."
