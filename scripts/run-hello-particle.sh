#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${REPO_ROOT}"
if [[ "$#" -eq 0 ]]; then
  set -- particle
fi

BENCHMARK=false
for argument in "$@"; do
  if [[ "${argument}" == "--bench" ]]; then
    BENCHMARK=true
    break
  fi
done

if [[ "${BENCHMARK}" == true ]]; then
  exec cargo run --release --package pqo-metal --bin hello-particle-view -- "$@"
fi

exec cargo run --package pqo-metal --bin hello-particle-view -- "$@"
