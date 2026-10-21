# Archive admission: verify the exact private bytes before extraction or execution.
fail() { printf 'INSTALL_ERROR: %s\n' "$*" >&2; exit 1; }
valid_pin() { [ "${#1}" -eq 64 ] && case "$1" in *[!0-9a-f]*) false;; *) true;; esac; }
sha256() {
    if command -v sha256sum >/dev/null 2>&1; then sha256sum "$1" | awk '{print $1}'
    elif command -v shasum >/dev/null 2>&1; then shasum -a 256 "$1" | awk '{print $1}'
    else fail 'SHA-256 utility required (sha256sum or shasum)'; fi
}
download() {
    curl --fail --location --silent --show-error --proto '=https' \
        --connect-timeout 20 --max-time 1800 "$1" -o "$2" || fail "download failed: $1"
}
native_platform() {
    case "$(uname -m)" in x86_64|amd64) cpu=x86_64;; arm64|aarch64) cpu=aarch64;; *) fail 'unsupported CPU';; esac
    case "$(uname -s)" in
        Linux)
            libc=$(getconf GNU_LIBC_VERSION) || fail 'GNU libc >= 2.39 required'
            printf '%s\n' "$libc" | awk '$1=="glibc" {split($2,v,"."); if(v[1]>2 || (v[1]==2 && v[2]>=39)) exit 0} {exit 1}' || fail 'GNU libc >= 2.39 required'
            printf 'linux-%s\n' "$cpu" ;;
        Darwin) printf 'macos-%s\n' "$cpu" ;;
        *) fail 'unsupported OS (use the PowerShell installer on Windows)' ;;
    esac
}
extract_archive() {
    tar -tzf "$work/archive.tar.gz" > "$work/names" || fail 'invalid archive listing'
    # Strict portable names, known roots, no traversal or ambiguous duplicate paths.
    awk '
        /[^a-zA-Z0-9_.+\/-]/ {exit 1}
        {sub(/\/$/, ""); n=split($0,p,"/");
         if(p[1]!="bundle" && p[1]!="runtime-sources" && p[1]!="coordinator-license-additions" && p[1]!="LICENSE" && p[1]!="NOTICE") exit 1;
         for(i=1;i<=n;i++) if(p[i]=="" || p[i]=="." || p[i]=="..") exit 1;
         if(seen[$0]++) exit 1; count++}
        END {if(!count) exit 1}' "$work/names" || fail 'unsafe archive paths'
    tar -tvzf "$work/archive.tar.gz" > "$work/types" || fail 'invalid archive types'
    awk 'substr($0,1,1)!="-" && substr($0,1,1)!="d" {exit 1}' "$work/types" || fail 'archive links/special files are forbidden'
    mkdir "$work/payload"
    tar -xzf "$work/archive.tar.gz" -C "$work/payload" --no-same-owner --no-same-permissions || fail 'archive extraction failed'
}
