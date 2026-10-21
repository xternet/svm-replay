#!/bin/sh
set -eu
umask 077
export LC_ALL=C
owner=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
. "$owner/_0_archive.sh"
. "$owner/_1_install.sh"
archive= pin=
prefix=${XDG_DATA_HOME:-"$HOME/.local/share"}/svm-replay/installations/0.2.0
bin_dir=$HOME/.local/bin
while [ "$#" -gt 0 ]; do
    case "$1" in
        --help|-h) printf '%s\n' 'Install SVM Replay 0.2.0 (no compiler required).' \
            'Options: --prefix NEW_DIRECTORY --bin-dir DIRECTORY' \
            'Offline: --archive FILE --sha256 TRUSTED_ARCHIVE_SHA256'; exit 0 ;;
        --archive|--sha256|--prefix|--bin-dir)
            [ "$#" -ge 2 ] || fail "missing value for $1"
            [ -n "$2" ] || fail "empty value for $1"
            case "$1" in --archive) archive=$2;; --sha256) pin=$2;;
                --prefix) prefix=$2;; --bin-dir) bin_dir=$2;; esac
            shift 2 ;;
        *) fail "unknown option: $1" ;;
    esac
done
case "$prefix:$bin_dir" in *'
'*) fail 'paths must not contain newlines';; esac
case "$prefix" in /*) ;; *) fail '--prefix must be absolute';; esac
case "$bin_dir" in /*) ;; *) fail '--bin-dir must be absolute';; esac
[ ! -e "$prefix" ] && [ ! -L "$prefix" ] || fail "installation already exists: $prefix"
[ ! -e "$bin_dir/svm-replay" ] && [ ! -L "$bin_dir/svm-replay" ] || fail "launcher already exists: $bin_dir/svm-replay"
if [ -n "$archive$pin" ]; then
    [ -n "$archive" ] && valid_pin "$pin" || fail '--archive and a 64-digit --sha256 are required together'
    [ -f "$archive" ] || fail "archive is not a regular file: $archive"
fi
work=$(mktemp -d "${TMPDIR:-/tmp}/svm-replay-install.XXXXXXXX")
trap 'rm -rf -- "$work"' EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
if [ -n "$archive" ]; then
    cp -- "$archive" "$work/archive.tar.gz"
else
    platform=$(native_platform)
    file=svm-replay-0.2.0-$platform.tar.gz
    base=https://github.com/xternet/svm-replay/releases/download/v0.2.0
    download "$base/SHA256SUMS" "$work/SHA256SUMS"
    pin=$(awk -v file="$file" '$2 == file {print $1; count++} END {if(count != 1) exit 1}' "$work/SHA256SUMS") || fail 'missing or ambiguous release checksum'
    valid_pin "$pin" || fail 'invalid release checksum'
    download "$base/$file" "$work/archive.tar.gz"
fi
[ "$(sha256 "$work/archive.tar.gz")" = "$pin" ] || fail 'archive integrity mismatch'
extract_archive
install_bundle
