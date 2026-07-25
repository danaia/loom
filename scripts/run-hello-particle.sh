#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"

cd "${REPO_ROOT}"
if [[ "$#" -eq 0 ]]; then
  set -- particle
fi
exec cargo run --package loom-metal --bin hello-particle-view -- "$@"
