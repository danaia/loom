#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${REPO_ROOT}"
if [[ "$#" -eq 0 ]]; then
  set -- particle
fi

CARGO_PROFILE=()
for argument in "$@"; do
  if [[ "${argument}" == "--bench" ]]; then
    CARGO_PROFILE=(--release)
    break
  fi
done

exec cargo run "${CARGO_PROFILE[@]}" --package loom-metal --bin hello-particle-view -- "$@"
