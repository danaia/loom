#!/bin/sh
set -eu

PQO_REPOSITORY="${PQO_REPOSITORY:-danaia/pqo}"
PQO_HOME="${PQO_HOME:-${HOME}/.pqo}"
PQO_BIN_DIR="${PQO_BIN_DIR:-${HOME}/.local/bin}"
PQO_VERSION="${PQO_VERSION:-}"

fail() {
  echo "pqo installer: $*" >&2
  exit 1
}

safe_pqo_home() {
  case "$1" in
    ""|"/"|"."|"${HOME}") return 1 ;;
    *) return 0 ;;
  esac
}

safe_pqo_home "${PQO_HOME}" || fail "refusing unsafe PQO_HOME: ${PQO_HOME}"

case "$(uname -s):$(uname -m)" in
  Darwin:arm64)
    PQO_PLATFORM="darwin"
    PQO_ARCH="arm64"
    ;;
  Linux:x86_64)
    PQO_PLATFORM="linux"
    PQO_ARCH="x86_64"
    ;;
  *)
    fail "Pqo requires macOS/Apple Silicon or Linux/x86_64 with an NVIDIA CUDA driver"
    ;;
esac

command -v curl >/dev/null 2>&1 || fail "curl is required"
command -v tar >/dev/null 2>&1 || fail "tar is required"

ASSET_NAME="pqo-${PQO_PLATFORM}-${PQO_ARCH}"
if [ -n "${PQO_ARCHIVE_URL:-}" ]; then
  ARCHIVE_URL="${PQO_ARCHIVE_URL}"
  CHECKSUM_URL="${PQO_CHECKSUM_URL:-${PQO_ARCHIVE_URL}.sha256}"
  INSTALL_SOURCE="local archive"
elif [ -n "${PQO_VERSION}" ]; then
  RELEASE_ROOT="https://github.com/${PQO_REPOSITORY}/releases/download/${PQO_VERSION}"
  ARCHIVE_URL="${RELEASE_ROOT}/${ASSET_NAME}.tar.gz"
  CHECKSUM_URL="${RELEASE_ROOT}/${ASSET_NAME}.sha256"
  INSTALL_SOURCE="Pqo ${PQO_VERSION} release"
else
  RELEASE_ROOT="https://github.com/${PQO_REPOSITORY}/releases/latest/download"
  RELEASE_CACHE_KEY="$(date +%s)"
  ARCHIVE_URL="${RELEASE_ROOT}/${ASSET_NAME}.tar.gz?pqo_release=${RELEASE_CACHE_KEY}"
  CHECKSUM_URL="${RELEASE_ROOT}/${ASSET_NAME}.sha256?pqo_release=${RELEASE_CACHE_KEY}"
  INSTALL_SOURCE="latest Pqo release"
fi

TEMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/pqo-install.XXXXXX")"
ARCHIVE_FILE="${TEMP_ROOT}/${ASSET_NAME}.tar.gz"
CHECKSUM_FILE="${TEMP_ROOT}/${ASSET_NAME}.sha256"

cleanup() {
  rm -rf "${TEMP_ROOT}"
}
trap cleanup EXIT HUP INT TERM

echo "Installing Pqo for ${PQO_PLATFORM} ${PQO_ARCH} from ${INSTALL_SOURCE}..."
curl -fL --retry 3 --proto '=https,file' --tlsv1.2 \
  "${ARCHIVE_URL}" -o "${ARCHIVE_FILE}"
curl -fL --retry 3 --proto '=https,file' --tlsv1.2 \
  "${CHECKSUM_URL}" -o "${CHECKSUM_FILE}"

EXPECTED_HASH="$(awk 'NR == 1 { print $1 }' "${CHECKSUM_FILE}")"
[ -n "${EXPECTED_HASH}" ] || fail "release checksum is empty"

if command -v shasum >/dev/null 2>&1; then
  ACTUAL_HASH="$(shasum -a 256 "${ARCHIVE_FILE}" | awk '{ print $1 }')"
elif command -v sha256sum >/dev/null 2>&1; then
  ACTUAL_HASH="$(sha256sum "${ARCHIVE_FILE}" | awk '{ print $1 }')"
else
  fail "shasum or sha256sum is required"
fi
[ "${EXPECTED_HASH}" = "${ACTUAL_HASH}" ] || fail "release checksum did not match"

tar -xzf "${ARCHIVE_FILE}" -C "${TEMP_ROOT}"
NEW_HOME="${TEMP_ROOT}/pqo"
[ -x "${NEW_HOME}/bin/pqo" ] || fail "release does not contain bin/pqo"
[ -f "${NEW_HOME}/install-manifest" ] || fail "release manifest is missing"
grep -q '^pqo-install-layout=1$' "${NEW_HOME}/install-manifest" ||
  fail "release layout is not recognized"

PQO_LINK="${PQO_BIN_DIR}/pqo"
if [ -e "${PQO_LINK}" ] || [ -L "${PQO_LINK}" ]; then
  if [ ! -L "${PQO_LINK}" ] ||
    [ "$(readlink "${PQO_LINK}")" != "${PQO_HOME}/bin/pqo" ]; then
    fail "${PQO_LINK} already exists and is not managed by Pqo"
  fi
fi

if [ -e "${PQO_HOME}" ]; then
  [ -f "${PQO_HOME}/install-manifest" ] ||
    fail "${PQO_HOME} exists but is not a managed Pqo installation"
  grep -q '^pqo-install-layout=1$' "${PQO_HOME}/install-manifest" ||
    fail "${PQO_HOME} has an unrecognized installation layout"
fi

mkdir -p "$(dirname "${PQO_HOME}")" "${PQO_BIN_DIR}"
NEXT_HOME="${PQO_HOME}.new.$$"
PREVIOUS_HOME="${PQO_HOME}.previous.$$"
rm -rf "${NEXT_HOME}" "${PREVIOUS_HOME}"
mv "${NEW_HOME}" "${NEXT_HOME}"

if [ -e "${PQO_HOME}" ]; then
  mv "${PQO_HOME}" "${PREVIOUS_HOME}"
fi
if ! mv "${NEXT_HOME}" "${PQO_HOME}"; then
  if [ -e "${PREVIOUS_HOME}" ]; then
    mv "${PREVIOUS_HOME}" "${PQO_HOME}"
  fi
  fail "could not activate the new Pqo installation"
fi

printf '%s\n' "bin_dir=${PQO_BIN_DIR}" >> "${PQO_HOME}/install-manifest"
rm -f "${PQO_LINK}"
ln -s "${PQO_HOME}/bin/pqo" "${PQO_LINK}"
rm -rf "${PREVIOUS_HOME}"

echo
echo "Pqo $("${PQO_HOME}/bin/pqo" --version | awk '{ print $2 }') installed."
echo "  home: ${PQO_HOME}"
echo "  command: ${PQO_LINK}"
echo "  particle: pqo ${PQO_HOME}/examples/hello-particle.pqo"
if [ "${PQO_PLATFORM}" = "linux" ]; then
  echo "  CUDA headless: PQO_HEADLESS_TICKS=120 pqo run ${PQO_HOME}/examples/hello-cuda.pqo --target cuda-headless"
  echo "  CUDA crystal: PQO_HEADLESS_TICKS=240 pqo run ${PQO_HOME}/examples/crystal-cuda.pqo --target cuda-headless"
fi
echo "  neon flock: pqo ${PQO_HOME}/examples/neon-flock.pqo"
echo "  crystal: pqo ${PQO_HOME}/examples/crystal.pqo"
echo "  marble water package: pqo ${PQO_HOME}/examples/marble-water.lmp"
echo "  update: pqo update"
if ! command -v pqo >/dev/null 2>&1; then
  echo
  echo "Add ${PQO_BIN_DIR} to PATH, then open a new terminal:"
  echo "  export PATH=\"${PQO_BIN_DIR}:\$PATH\""
fi
