#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
DIST_DIR="${PQO_DIST_DIR:-${REPO_ROOT}/target/dist}"
HOST_OS="$(uname -s)"
HOST_ARCH="$(uname -m)"

case "${HOST_OS}:${HOST_ARCH}" in
  Darwin:arm64)
    PACKAGE_PLATFORM="darwin"
    PACKAGE_ARCH="arm64"
    ;;
  Linux:x86_64)
    PACKAGE_PLATFORM="linux"
    PACKAGE_ARCH="x86_64"
    ;;
  *)
    echo "error: Pqo runtime packages support macOS/Apple Silicon or Linux/x86_64 with NVIDIA CUDA" >&2
    exit 1
    ;;
esac

VERSION="$(
  sed -n '/^\[workspace\.package\]/,/^\[/s/^version = "\([^"]*\)"/\1/p' \
    "${REPO_ROOT}/Cargo.toml"
)"
if [[ -z "${VERSION}" ]]; then
  echo "error: could not read the Pqo workspace version" >&2
  exit 1
fi

ASSET_NAME="pqo-${PACKAGE_PLATFORM}-${PACKAGE_ARCH}"
ARCHIVE_PATH="${DIST_DIR}/${ASSET_NAME}.tar.gz"
STAGING_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/pqo-package.XXXXXX")"
PACKAGE_ROOT="${STAGING_ROOT}/pqo"

cleanup() {
  rm -rf "${STAGING_ROOT}"
}
trap cleanup EXIT

cd "${REPO_ROOT}"
cargo build --locked --release --package pqo-cli --package pqo-ui-panel

mkdir -p \
  "${PACKAGE_ROOT}/bin" \
  "${PACKAGE_ROOT}/baseline" \
  "${PACKAGE_ROOT}/examples" \
  "${PACKAGE_ROOT}/examples/kernels" \
  "${PACKAGE_ROOT}/examples/ui-crystal-cuda/dist" \
  "${PACKAGE_ROOT}/share/pqo/docs/releases"

install -m 0755 "${REPO_ROOT}/target/release/pqo" "${PACKAGE_ROOT}/bin/pqo"
install -m 0755 \
  "${REPO_ROOT}/target/release/pqo-ui-panel" \
  "${PACKAGE_ROOT}/bin/pqo-ui-panel"
install -m 0755 "${REPO_ROOT}/uninstall.sh" "${PACKAGE_ROOT}/uninstall"
tar -C "${REPO_ROOT}" -cf - \
  --exclude='baseline/.pqo' \
  --exclude='baseline/agentDB' \
  --exclude='baseline/ui/node_modules' \
  baseline | \
  tar -C "${PACKAGE_ROOT}" -xf -
install -m 0644 \
  "${REPO_ROOT}/examples/hello-particle/hello-particle.pqo" \
  "${PACKAGE_ROOT}/examples/hello-particle.pqo"
install -m 0644 \
  "${REPO_ROOT}/examples/hello-crystal/crystal.pqo" \
  "${PACKAGE_ROOT}/examples/crystal.pqo"
install -m 0644 \
  "${REPO_ROOT}/examples/hello-crystal/crystal-cuda.pqo" \
  "${PACKAGE_ROOT}/examples/crystal-cuda.pqo"
install -m 0644 \
  "${REPO_ROOT}/examples/hello-crystal/kernels/crystal-cuda.cu" \
  "${PACKAGE_ROOT}/examples/kernels/crystal-cuda.cu"
install -m 0644 \
  "${REPO_ROOT}/examples/hello-crystal/ui-crystal-cuda/pqo-ui.json" \
  "${PACKAGE_ROOT}/examples/ui-crystal-cuda/pqo-ui.json"
install -m 0644 \
  "${REPO_ROOT}/examples/hello-crystal/ui-crystal-cuda/dist/index.html" \
  "${PACKAGE_ROOT}/examples/ui-crystal-cuda/dist/index.html"
install -m 0644 \
  "${REPO_ROOT}/examples/hello-crystal/ui-crystal-cuda/dist/styles.css" \
  "${PACKAGE_ROOT}/examples/ui-crystal-cuda/dist/styles.css"
install -m 0644 \
  "${REPO_ROOT}/examples/hello-crystal/ui-crystal-cuda/dist/app.js" \
  "${PACKAGE_ROOT}/examples/ui-crystal-cuda/dist/app.js"
install -m 0644 \
  "${REPO_ROOT}/examples/hello-cuda/hello-cuda.pqo" \
  "${PACKAGE_ROOT}/examples/hello-cuda.pqo"
install -m 0644 \
  "${REPO_ROOT}/examples/neon-flock/neon-flock.pqo" \
  "${PACKAGE_ROOT}/examples/neon-flock.pqo"
install -m 0644 \
  "${REPO_ROOT}/examples/marble-water/marble-water.lmp" \
  "${PACKAGE_ROOT}/examples/marble-water.lmp"
install -m 0644 \
  "${REPO_ROOT}/examples/README.md" \
  "${PACKAGE_ROOT}/examples/README.md"
install -m 0644 \
  "${REPO_ROOT}/examples/hello-particle/README.md" \
  "${PACKAGE_ROOT}/examples/hello-particle.README.md"
install -m 0644 \
  "${REPO_ROOT}/examples/hello-crystal/README.md" \
  "${PACKAGE_ROOT}/examples/crystal.README.md"
install -m 0644 \
  "${REPO_ROOT}/examples/hello-crystal/README-cuda.md" \
  "${PACKAGE_ROOT}/examples/crystal-cuda.README.md"
install -m 0644 \
  "${REPO_ROOT}/examples/neon-flock/README.md" \
  "${PACKAGE_ROOT}/examples/neon-flock.README.md"
install -m 0644 \
  "${REPO_ROOT}/examples/marble-water/README.md" \
  "${PACKAGE_ROOT}/examples/marble-water.README.md"
cp -R "${REPO_ROOT}/docs/handbook" "${PACKAGE_ROOT}/share/pqo/docs/handbook"
install -m 0644 "${REPO_ROOT}/docs/README.md" "${PACKAGE_ROOT}/share/pqo/docs/README.md"
install -m 0644 \
  "${REPO_ROOT}/docs/agent-coding-reference.md" \
  "${PACKAGE_ROOT}/share/pqo/docs/agent-coding-reference.md"
install -m 0644 \
  "${REPO_ROOT}/docs/native-compiler-gates.md" \
  "${PACKAGE_ROOT}/share/pqo/docs/native-compiler-gates.md"
install -m 0644 \
  "${REPO_ROOT}/docs/package-format.md" \
  "${PACKAGE_ROOT}/share/pqo/docs/package-format.md"
install -m 0644 \
  "${REPO_ROOT}/docs/releases/v${VERSION}.md" \
  "${PACKAGE_ROOT}/share/pqo/docs/releases/v${VERSION}.md"

printf '%s\n' "${VERSION}" > "${PACKAGE_ROOT}/VERSION"
{
  printf '%s\n' "pqo-install-layout=1"
  printf '%s\n' "version=${VERSION}"
  printf '%s\n' "platform=${PACKAGE_PLATFORM}"
  printf '%s\n' "architecture=${PACKAGE_ARCH}"
} > "${PACKAGE_ROOT}/install-manifest"

mkdir -p "${DIST_DIR}"
tar -C "${STAGING_ROOT}" -czf "${ARCHIVE_PATH}" pqo

if command -v shasum >/dev/null 2>&1; then
  (
    cd "${DIST_DIR}"
    shasum -a 256 "$(basename "${ARCHIVE_PATH}")" \
      > "${ASSET_NAME}.sha256"
  )
else
  (
    cd "${DIST_DIR}"
    sha256sum "$(basename "${ARCHIVE_PATH}")" \
      > "${ASSET_NAME}.sha256"
  )
fi

echo "Packaged ${ARCHIVE_PATH}"
