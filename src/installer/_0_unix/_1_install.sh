# Native code owns bundle validation and installation, including every worker pin.
install_bundle() {
    bundle=$work/payload/bundle
    binary=$bundle/bin/svm-replay
    [ -f "$binary" ] && [ -x "$binary" ] || fail 'missing executable native CLI binary'
    [ -f "$bundle/bundle.json" ] || fail 'missing bundle manifest'
    bundle_pin=$(sha256 "$bundle/bundle.json")
    "$binary" doctor --bundle "$bundle/bundle.json" --bundle-sha256 "$bundle_pin" || fail 'bundle verification failed'
    mkdir -p -- "$(dirname -- "$prefix")"
    "$binary" bundle install --bundle "$bundle/bundle.json" --bundle-sha256 "$bundle_pin" --output "$prefix" || fail "native installation failed; inspect partial directory: $prefix"
    for item in runtime-sources coordinator-license-additions LICENSE NOTICE; do
        if [ -e "$work/payload/$item" ]; then
            cp -R -- "$work/payload/$item" "$prefix/$item" || fail "cannot preserve $item; installation not activated"
        fi
    done
    "$prefix/bin/svm-replay" doctor || fail 'installed bundle verification failed; installation not activated'
    mkdir -p -- "$bin_dir"
    quoted=$(printf '%s' "$prefix/bin/svm-replay" | sed "s/'/'\\\\''/g")
    (set -C; printf '#!/bin/sh\nexec '\''%s'\'' "$@"\n' "$quoted" > "$bin_dir/svm-replay") || fail 'launcher already exists or cannot be created'
    chmod 755 "$bin_dir/svm-replay" || fail 'cannot make launcher executable'
    printf '\nInstalled SVM Replay 0.2.0: %s\n' "$prefix"
    printf 'Add this directory to PATH if needed: %s\nThen run: svm-replay demo\n' "$bin_dir"
}
