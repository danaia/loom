#!/bin/sh
set -eu

LOOM_BACKEND="${LOOM_BACKEND:-auto}"

fail() {
  echo "loom remover: $*" >&2
  exit 1
}

if [ -z "${LOOM_HOME:-}" ]; then
  case "${LOOM_BACKEND}" in
    metal|cuda) LOOM_HOME="${HOME}/.loom-${LOOM_BACKEND}" ;;
    auto)
      if [ -e "${HOME}/.loom" ]; then
        LOOM_HOME="${HOME}/.loom"
      elif [ "$(uname -s)" = "Linux" ]; then
        LOOM_HOME="${HOME}/.loom-cuda"
      else
        LOOM_HOME="${HOME}/.loom-metal"
      fi
      ;;
    *) fail "unsupported LOOM_BACKEND=${LOOM_BACKEND}; expected metal, cuda, or auto" ;;
  esac
fi

case "${LOOM_HOME}" in
  ""|"/"|"."|"${HOME}") fail "refusing unsafe LOOM_HOME: ${LOOM_HOME}" ;;
esac

if [ ! -e "${LOOM_HOME}" ]; then
  echo "Loom is not installed at ${LOOM_HOME}."
  exit 0
fi

MANIFEST="${LOOM_HOME}/install-manifest"
[ -f "${MANIFEST}" ] ||
  fail "${LOOM_HOME} is not a managed Loom installation; nothing was removed"
grep -q '^loom-install-layout=1$' "${MANIFEST}" ||
  fail "${LOOM_HOME} has an unrecognized layout; nothing was removed"

LOOM_BIN_DIR="$(
  sed -n 's/^bin_dir=//p' "${MANIFEST}" | tail -n 1
)"
LOOM_BIN_DIR="${LOOM_BIN_DIR:-${HOME}/.local/bin}"
LOOM_BACKEND="$(
  sed -n 's/^backend=//p' "${MANIFEST}" | tail -n 1
)"
LOOM_ALIAS="${LOOM_BIN_DIR}/loom-${LOOM_BACKEND:-metal}"
LOOM_LINK="${LOOM_BIN_DIR}/loom"

for LINK in "${LOOM_ALIAS}" "${LOOM_LINK}"; do
  if [ -L "${LINK}" ] &&
    [ "$(readlink "${LINK}")" = "${LOOM_HOME}/bin/loom" ]; then
    rm "${LINK}"
  elif [ -e "${LINK}" ] || [ -L "${LINK}" ]; then
    echo "loom remover: leaving unrelated ${LINK} in place" >&2
  fi
done

rm -rf "${LOOM_HOME}"
echo "Removed Loom from ${LOOM_HOME}."
