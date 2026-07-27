#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/.." && pwd)"
DIST_DIR="${LOOM_DIST_DIR:-${REPO_ROOT}/target/dist}"
HOST_OS="$(uname -s)"
HOST_ARCH="$(uname -m)"

if [[ "${HOST_OS}" != "Darwin" ]]; then
  echo "error: Loom runtime packages currently require macOS and Metal" >&2
  exit 1
fi

if [[ "${HOST_ARCH}" != "arm64" ]]; then
  echo "error: Loom release packages require Apple Silicon (arm64)" >&2
  exit 1
fi
PACKAGE_ARCH="arm64"

VERSION="$(
  sed -n '/^\[workspace\.package\]/,/^\[/s/^version = "\([^"]*\)"/\1/p' \
    "${REPO_ROOT}/Cargo.toml"
)"
if [[ -z "${VERSION}" ]]; then
  echo "error: could not read the Loom workspace version" >&2
  exit 1
fi

ASSET_NAME="loom-darwin-${PACKAGE_ARCH}"
ARCHIVE_PATH="${DIST_DIR}/${ASSET_NAME}.tar.gz"
STAGING_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/loom-package.XXXXXX")"
PACKAGE_ROOT="${STAGING_ROOT}/loom"

cleanup() {
  rm -rf "${STAGING_ROOT}"
}
trap cleanup EXIT

cd "${REPO_ROOT}"
cargo build --locked --release --package loom-cli --package loom-ui-panel
command -v npm >/dev/null 2>&1 || {
  echo "error: npm is required to package the Marble Water project UI" >&2
  exit 1
}
npm install --prefix "${REPO_ROOT}/examples/marble-water/ui" --no-audit --no-fund
"${REPO_ROOT}/target/release/loom" build \
  "${REPO_ROOT}/examples/marble-water/marble-water.loom"

mkdir -p \
  "${PACKAGE_ROOT}/bin" \
  "${PACKAGE_ROOT}/baseline" \
  "${PACKAGE_ROOT}/examples" \
  "${PACKAGE_ROOT}/share/loom/docs/releases"

install -m 0755 "${REPO_ROOT}/target/release/loom" "${PACKAGE_ROOT}/bin/loom"
install -m 0755 \
  "${REPO_ROOT}/target/release/loom-ui-panel" \
  "${PACKAGE_ROOT}/bin/loom-ui-panel"
install -m 0755 "${REPO_ROOT}/uninstall.sh" "${PACKAGE_ROOT}/uninstall"
tar -C "${REPO_ROOT}" -cf - --exclude='baseline/ui/node_modules' baseline | \
  tar -C "${PACKAGE_ROOT}" -xf -
install -m 0644 \
  "${REPO_ROOT}/examples/hello-particle/hello-particle.loom" \
  "${PACKAGE_ROOT}/examples/hello-particle.loom"
install -m 0644 \
  "${REPO_ROOT}/examples/hello-crystal/crystal.loom" \
  "${PACKAGE_ROOT}/examples/crystal.loom"
install -m 0644 \
  "${REPO_ROOT}/examples/neon-flock/neon-flock.loom" \
  "${PACKAGE_ROOT}/examples/neon-flock.loom"
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
  "${REPO_ROOT}/examples/neon-flock/README.md" \
  "${PACKAGE_ROOT}/examples/neon-flock.README.md"
install -m 0644 \
  "${REPO_ROOT}/examples/marble-water/README.md" \
  "${PACKAGE_ROOT}/examples/marble-water.README.md"
cp -R "${REPO_ROOT}/docs/handbook" "${PACKAGE_ROOT}/share/loom/docs/handbook"
install -m 0644 "${REPO_ROOT}/docs/README.md" "${PACKAGE_ROOT}/share/loom/docs/README.md"
install -m 0644 \
  "${REPO_ROOT}/docs/agent-coding-reference.md" \
  "${PACKAGE_ROOT}/share/loom/docs/agent-coding-reference.md"
install -m 0644 \
  "${REPO_ROOT}/docs/native-compiler-gates.md" \
  "${PACKAGE_ROOT}/share/loom/docs/native-compiler-gates.md"
install -m 0644 \
  "${REPO_ROOT}/docs/package-format.md" \
  "${PACKAGE_ROOT}/share/loom/docs/package-format.md"
install -m 0644 \
  "${REPO_ROOT}/docs/releases/v${VERSION}.md" \
  "${PACKAGE_ROOT}/share/loom/docs/releases/v${VERSION}.md"

printf '%s\n' "${VERSION}" > "${PACKAGE_ROOT}/VERSION"
{
  printf '%s\n' "loom-install-layout=1"
  printf '%s\n' "version=${VERSION}"
  printf '%s\n' "platform=darwin"
  printf '%s\n' "architecture=${PACKAGE_ARCH}"
} > "${PACKAGE_ROOT}/install-manifest"

mkdir -p "${DIST_DIR}"
tar -C "${STAGING_ROOT}" -czf "${ARCHIVE_PATH}" loom

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
