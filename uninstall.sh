#!/bin/sh
set -eu

PQO_HOME="${PQO_HOME:-${HOME}/.pqo}"

fail() {
  echo "pqo remover: $*" >&2
  exit 1
}

case "${PQO_HOME}" in
  ""|"/"|"."|"${HOME}") fail "refusing unsafe PQO_HOME: ${PQO_HOME}" ;;
esac

if [ ! -e "${PQO_HOME}" ]; then
  echo "Pqo is not installed at ${PQO_HOME}."
  exit 0
fi

MANIFEST="${PQO_HOME}/install-manifest"
[ -f "${MANIFEST}" ] ||
  fail "${PQO_HOME} is not a managed Pqo installation; nothing was removed"
grep -q '^pqo-install-layout=1$' "${MANIFEST}" ||
  fail "${PQO_HOME} has an unrecognized layout; nothing was removed"

PQO_BIN_DIR="$(
  sed -n 's/^bin_dir=//p' "${MANIFEST}" | tail -n 1
)"
PQO_BIN_DIR="${PQO_BIN_DIR:-${HOME}/.local/bin}"
PQO_LINK="${PQO_BIN_DIR}/pqo"

if [ -L "${PQO_LINK}" ] &&
  [ "$(readlink "${PQO_LINK}")" = "${PQO_HOME}/bin/pqo" ]; then
  rm "${PQO_LINK}"
elif [ -e "${PQO_LINK}" ] || [ -L "${PQO_LINK}" ]; then
  echo "pqo remover: leaving unrelated ${PQO_LINK} in place" >&2
fi

rm -rf "${PQO_HOME}"
echo "Removed Pqo from ${PQO_HOME}."
