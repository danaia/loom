#!/bin/sh
set -eu

LOOM_HOME="${LOOM_HOME:-${HOME}/.loom}"

fail() {
  echo "loom remover: $*" >&2
  exit 1
}

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
LOOM_LINK="${LOOM_BIN_DIR}/loom"

if [ -L "${LOOM_LINK}" ] &&
  [ "$(readlink "${LOOM_LINK}")" = "${LOOM_HOME}/bin/loom" ]; then
  rm "${LOOM_LINK}"
elif [ -e "${LOOM_LINK}" ] || [ -L "${LOOM_LINK}" ]; then
  echo "loom remover: leaving unrelated ${LOOM_LINK} in place" >&2
fi

rm -rf "${LOOM_HOME}"
echo "Removed Loom from ${LOOM_HOME}."
