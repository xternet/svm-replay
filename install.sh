#!/bin/sh
# Thin entry; implementation lives in src/installer/_0_unix/.
set -eu
if [ -f "$0" ]; then
    owner=$(CDPATH= cd -- "$(dirname -- "$0")/src/installer/_0_unix" && pwd)
    exec sh "$owner/main.sh" "$@"
fi
owner=$(mktemp -d "${TMPDIR:-/tmp}/svm-replay-bootstrap.XXXXXXXX")
cleanup() { rm -rf -- "$owner"; }
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
base=https://raw.githubusercontent.com/xternet/svm-replay/v0.2.0/src/installer/_0_unix
for file in main.sh _0_archive.sh _1_install.sh; do
    curl --fail --location --silent --show-error --proto '=https' \
        --connect-timeout 20 --max-time 120 "$base/$file" -o "$owner/$file"
done
sh "$owner/main.sh" "$@"
