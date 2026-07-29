#!/bin/sh
set -eu

LOOM_REPOSITORY="${LOOM_REPOSITORY:-danaia/loom}"
LOOM_BIN_DIR="${LOOM_BIN_DIR:-${HOME}/.local/bin}"
LOOM_VERSION="${LOOM_VERSION:-}"
LOOM_BACKEND="${LOOM_BACKEND:-auto}"
LOOM_SET_DEFAULT="${LOOM_SET_DEFAULT:-1}"

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

HOST_OS="$(uname -s)"
HOST_ARCH="$(uname -m)"

case "${HOST_OS}" in
  Darwin) LOOM_PLATFORM="darwin" ;;
  Linux) LOOM_PLATFORM="linux" ;;
  *) fail "unsupported OS ${HOST_OS}; expected Darwin or Linux" ;;
esac

case "${HOST_ARCH}" in
  arm64|aarch64) LOOM_ARCH="arm64" ;;
  x86_64|amd64) LOOM_ARCH="x86_64" ;;
  *) fail "unsupported architecture ${HOST_ARCH}" ;;
esac

case "${LOOM_BACKEND}" in
  auto)
    if [ "${LOOM_PLATFORM}" = "darwin" ] && [ "${LOOM_ARCH}" = "arm64" ]; then
      LOOM_BACKEND="metal"
    elif [ "${LOOM_PLATFORM}" = "linux" ] && [ "${LOOM_ARCH}" = "x86_64" ]; then
      LOOM_BACKEND="cuda"
    else
      fail "could not infer Loom backend for ${LOOM_PLATFORM}-${LOOM_ARCH}; set LOOM_BACKEND=metal or LOOM_BACKEND=cuda"
    fi
    ;;
  metal|cuda) ;;
  *) fail "unsupported LOOM_BACKEND=${LOOM_BACKEND}; expected metal, cuda, or auto" ;;
esac

if [ "${LOOM_BACKEND}" = "metal" ]; then
  [ "${LOOM_PLATFORM}" = "darwin" ] ||
    fail "Loom Metal releases currently install on macOS only"
  [ "${LOOM_ARCH}" = "arm64" ] ||
    fail "Loom Metal releases currently require Apple Silicon (arm64)"
elif [ "${LOOM_BACKEND}" = "cuda" ]; then
  [ "${LOOM_PLATFORM}" = "linux" ] ||
    fail "Loom CUDA releases currently install on Linux only"
  [ "${LOOM_ARCH}" = "x86_64" ] ||
    fail "Loom CUDA releases currently require x86_64"
fi

LOOM_HOME="${LOOM_HOME:-${HOME}/.loom-${LOOM_BACKEND}}"
safe_loom_home "${LOOM_HOME}" || fail "refusing unsafe LOOM_HOME: ${LOOM_HOME}"

command -v curl >/dev/null 2>&1 || fail "curl is required"
command -v tar >/dev/null 2>&1 || fail "tar is required"

ASSET_NAME="loom-${LOOM_BACKEND}-${LOOM_PLATFORM}-${LOOM_ARCH}"
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

echo "Downloading Loom ${LOOM_BACKEND} for ${LOOM_PLATFORM} ${LOOM_ARCH}..."
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
grep -q "^backend=${LOOM_BACKEND}$" "${NEW_HOME}/install-manifest" ||
  fail "release backend does not match requested ${LOOM_BACKEND}"

LOOM_ALIAS_LINK="${LOOM_BIN_DIR}/loom-${LOOM_BACKEND}"
LOOM_DEFAULT_LINK="${LOOM_BIN_DIR}/loom"
LOOM_LINK="${LOOM_DEFAULT_LINK}"
if [ "${LOOM_SET_DEFAULT}" = "0" ]; then
  LOOM_LINK="${LOOM_ALIAS_LINK}"
fi

if [ -e "${LOOM_ALIAS_LINK}" ] || [ -L "${LOOM_ALIAS_LINK}" ]; then
  if [ ! -L "${LOOM_ALIAS_LINK}" ] ||
    [ "$(readlink "${LOOM_ALIAS_LINK}")" != "${LOOM_HOME}/bin/loom" ]; then
    fail "${LOOM_ALIAS_LINK} already exists and is not managed by Loom ${LOOM_BACKEND}"
  fi
fi

if [ -e "${LOOM_LINK}" ] || [ -L "${LOOM_LINK}" ]; then
  if [ ! -L "${LOOM_LINK}" ] ||
    [ "$(readlink "${LOOM_LINK}")" != "${LOOM_HOME}/bin/loom" ]; then
    if [ "${LOOM_LINK}" = "${LOOM_DEFAULT_LINK}" ] && [ -L "${LOOM_LINK}" ]; then
      CURRENT_TARGET="$(readlink "${LOOM_LINK}")"
      CURRENT_HOME="$(dirname "$(dirname "${CURRENT_TARGET}")")"
      if [ ! -f "${CURRENT_HOME}/install-manifest" ] ||
        ! grep -q '^loom-install-layout=1$' "${CURRENT_HOME}/install-manifest"; then
        fail "${LOOM_LINK} already exists and is not managed by Loom"
      fi
    else
      fail "${LOOM_LINK} already exists and is not managed by Loom"
    fi
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
rm -f "${LOOM_ALIAS_LINK}"
ln -s "${LOOM_HOME}/bin/loom" "${LOOM_ALIAS_LINK}"
if [ "${LOOM_SET_DEFAULT}" != "0" ]; then
  rm -f "${LOOM_DEFAULT_LINK}"
  ln -s "${LOOM_HOME}/bin/loom" "${LOOM_DEFAULT_LINK}"
fi
rm -rf "${PREVIOUS_HOME}"

echo
echo "Loom ${LOOM_BACKEND} $("${LOOM_HOME}/bin/loom" --version | awk '{ print $2 }') installed."
echo "  home: ${LOOM_HOME}"
echo "  backend command: ${LOOM_ALIAS_LINK}"
if [ "${LOOM_SET_DEFAULT}" != "0" ]; then
  echo "  default command: ${LOOM_DEFAULT_LINK}"
fi
if [ "${LOOM_BACKEND}" = "cuda" ]; then
  echo "  cuda baseline: loom-cuda ${LOOM_HOME}/baseline/baseline.cuda.loom"
  echo "  cuda check: loom-cuda check ${LOOM_HOME}/baseline/baseline.cuda.loom"
  echo "  cuda explain: loom-cuda explain ${LOOM_HOME}/baseline/baseline.cuda.loom"
else
  echo "  metal baseline: loom-metal ${LOOM_HOME}/baseline/baseline.loom"
  echo "  particle: loom-metal ${LOOM_HOME}/examples/hello-particle.loom"
  echo "  neon flock: loom-metal ${LOOM_HOME}/examples/neon-flock.loom"
  echo "  crystal: loom-metal ${LOOM_HOME}/examples/crystal.loom"
  echo "  marble water package: loom-metal ${LOOM_HOME}/examples/marble-water.lmp"
fi
echo "  update: loom-${LOOM_BACKEND} update"
if ! command -v "loom-${LOOM_BACKEND}" >/dev/null 2>&1; then
  echo
  echo "Add ${LOOM_BIN_DIR} to PATH, then open a new terminal:"
  echo "  export PATH=\"${LOOM_BIN_DIR}:\$PATH\""
fi
