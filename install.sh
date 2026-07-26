#!/bin/sh
set -eu

LOOM_REPOSITORY="${LOOM_REPOSITORY:-danaia/loom}"
LOOM_HOME="${LOOM_HOME:-${HOME}/.loom}"
LOOM_BIN_DIR="${LOOM_BIN_DIR:-${HOME}/.local/bin}"
LOOM_VERSION="${LOOM_VERSION:-}"

fail() {
  echo "loom installer: $*" >&2
  exit 1
}

safe_loom_home() {
  case "$1" in
    ""|"/"|"."|"${HOME}") return 1 ;;
    *) return 0 ;;
  esac
}

safe_loom_home "${LOOM_HOME}" || fail "refusing unsafe LOOM_HOME: ${LOOM_HOME}"

[ "$(uname -s)" = "Darwin" ] ||
  fail "Loom execution currently requires macOS and Metal"

[ "$(uname -m)" = "arm64" ] ||
  fail "Loom currently requires an Apple Silicon Mac (arm64)"
LOOM_ARCH="arm64"

command -v curl >/dev/null 2>&1 || fail "curl is required"
command -v tar >/dev/null 2>&1 || fail "tar is required"

ASSET_NAME="loom-darwin-${LOOM_ARCH}"
if [ -n "${LOOM_ARCHIVE_URL:-}" ]; then
  ARCHIVE_URL="${LOOM_ARCHIVE_URL}"
  CHECKSUM_URL="${LOOM_CHECKSUM_URL:-${LOOM_ARCHIVE_URL}.sha256}"
elif [ -n "${LOOM_VERSION}" ]; then
  RELEASE_ROOT="https://github.com/${LOOM_REPOSITORY}/releases/download/${LOOM_VERSION}"
  ARCHIVE_URL="${RELEASE_ROOT}/${ASSET_NAME}.tar.gz"
  CHECKSUM_URL="${RELEASE_ROOT}/${ASSET_NAME}.sha256"
else
  RELEASE_ROOT="https://github.com/${LOOM_REPOSITORY}/releases/latest/download"
  RELEASE_CACHE_KEY="$(date +%s)"
  ARCHIVE_URL="${RELEASE_ROOT}/${ASSET_NAME}.tar.gz?loom_release=${RELEASE_CACHE_KEY}"
  CHECKSUM_URL="${RELEASE_ROOT}/${ASSET_NAME}.sha256?loom_release=${RELEASE_CACHE_KEY}"
fi

TEMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/loom-install.XXXXXX")"
ARCHIVE_FILE="${TEMP_ROOT}/${ASSET_NAME}.tar.gz"
CHECKSUM_FILE="${TEMP_ROOT}/${ASSET_NAME}.sha256"

cleanup() {
  rm -rf "${TEMP_ROOT}"
}
trap cleanup EXIT HUP INT TERM

echo "Downloading Loom for macOS ${LOOM_ARCH}..."
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
NEW_HOME="${TEMP_ROOT}/loom"
[ -x "${NEW_HOME}/bin/loom" ] || fail "release does not contain bin/loom"
[ -f "${NEW_HOME}/install-manifest" ] || fail "release manifest is missing"
grep -q '^loom-install-layout=1$' "${NEW_HOME}/install-manifest" ||
  fail "release layout is not recognized"

LOOM_LINK="${LOOM_BIN_DIR}/loom"
if [ -e "${LOOM_LINK}" ] || [ -L "${LOOM_LINK}" ]; then
  if [ ! -L "${LOOM_LINK}" ] ||
    [ "$(readlink "${LOOM_LINK}")" != "${LOOM_HOME}/bin/loom" ]; then
    fail "${LOOM_LINK} already exists and is not managed by Loom"
  fi
fi

if [ -e "${LOOM_HOME}" ]; then
  [ -f "${LOOM_HOME}/install-manifest" ] ||
    fail "${LOOM_HOME} exists but is not a managed Loom installation"
  grep -q '^loom-install-layout=1$' "${LOOM_HOME}/install-manifest" ||
    fail "${LOOM_HOME} has an unrecognized installation layout"
fi

mkdir -p "$(dirname "${LOOM_HOME}")" "${LOOM_BIN_DIR}"
NEXT_HOME="${LOOM_HOME}.new.$$"
PREVIOUS_HOME="${LOOM_HOME}.previous.$$"
rm -rf "${NEXT_HOME}" "${PREVIOUS_HOME}"
mv "${NEW_HOME}" "${NEXT_HOME}"

if [ -e "${LOOM_HOME}" ]; then
  mv "${LOOM_HOME}" "${PREVIOUS_HOME}"
fi
if ! mv "${NEXT_HOME}" "${LOOM_HOME}"; then
  if [ -e "${PREVIOUS_HOME}" ]; then
    mv "${PREVIOUS_HOME}" "${LOOM_HOME}"
  fi
  fail "could not activate the new Loom installation"
fi

printf '%s\n' "bin_dir=${LOOM_BIN_DIR}" >> "${LOOM_HOME}/install-manifest"
rm -f "${LOOM_LINK}"
ln -s "${LOOM_HOME}/bin/loom" "${LOOM_LINK}"
rm -rf "${PREVIOUS_HOME}"

echo
echo "Loom $("${LOOM_HOME}/bin/loom" --version | awk '{ print $2 }') installed."
echo "  home: ${LOOM_HOME}"
echo "  command: ${LOOM_LINK}"
echo "  particle: loom ${LOOM_HOME}/examples/hello-particle.loom"
echo "  neon flock: loom ${LOOM_HOME}/examples/neon-flock.loom"
echo "  crystal: loom ${LOOM_HOME}/examples/crystal.loom"
echo "  update: loom update"
if ! command -v loom >/dev/null 2>&1; then
  echo
  echo "Add ${LOOM_BIN_DIR} to PATH, then open a new terminal:"
  echo "  export PATH=\"${LOOM_BIN_DIR}:\$PATH\""
fi
