#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
DIST_DIR="${PQO_DIST_DIR:-${REPO_ROOT}/target/dist}"
HOST_ARCH="$(uname -m)"
case "$(uname -s)" in
  Darwin) PLATFORM="darwin" ;;
  Linux) PLATFORM="linux" ;;
  *)
    echo "error: Pqo runtime packages support macOS/Apple Silicon or Linux/x86_64 with NVIDIA CUDA" >&2
    exit 1
    ;;
esac
ASSET_NAME="pqo-${PLATFORM}-${HOST_ARCH}"
ARCHIVE_PATH="${DIST_DIR}/${ASSET_NAME}.tar.gz"
CHECKSUM_PATH="${DIST_DIR}/${ASSET_NAME}.sha256"

"${SCRIPT_DIR}/package-pqo.sh"

[[ -f "${ARCHIVE_PATH}" ]] || {
  echo "error: package archive was not created at ${ARCHIVE_PATH}" >&2
  exit 1
}
[[ -f "${CHECKSUM_PATH}" ]] || {
  echo "error: package checksum was not created at ${CHECKSUM_PATH}" >&2
  exit 1
}

echo "Installing the Pqo build from this repository..."
PQO_ARCHIVE_URL="file://${ARCHIVE_PATH}" \
PQO_CHECKSUM_URL="file://${CHECKSUM_PATH}" \
  /bin/sh "${REPO_ROOT}/install.sh"
