#!/bin/bash
set -euo pipefail
scripts=$1
native=$2
reports=$3
export TOOL_WSL_KERNEL=$4
export TOOL_WORK
TOOL_WORK=$(mktemp -d /tmp/svm-tool-wsl.XXXXXXXX)
copy_reports() {
    result=$?
    if [ -d "$TOOL_WORK/reports" ]; then cp -R "$TOOL_WORK/reports/." "$reports/"; fi
    exit "$result"
}
trap copy_reports EXIT
apt-get update
apt-get install -y --no-install-recommends ca-certificates curl python3 iproute2 xz-utils
node_archive=node-v22.22.0-linux-x64.tar.xz
curl -fLsS --max-time 180 "https://nodejs.org/dist/v22.22.0/$node_archive" -o "$TOOL_WORK/$node_archive"
printf '%s  %s\n' 9aa8e9d2298ab68c600bd6fb86a6c13bce11a4eca1ba9b39d79fa021755d7c37 "$TOOL_WORK/$node_archive" | sha256sum --check --strict
mkdir "$TOOL_WORK/node"
tar -xJf "$TOOL_WORK/$node_archive" --strip-components=1 -C "$TOOL_WORK/node"
export PATH="$TOOL_WORK/node/bin:$PATH"
python3 "$scripts/prepare.py" linux-x86_64 "$native" "$TOOL_WORK/test"
export TOOL_WORK="$TOOL_WORK/test"
curl -fLsS --max-time 180 https://github.com/xternet/svm-replay/releases/download/validation-inputs-0.2.0/linux-tool-validation.tar.gz -o "$TOOL_WORK/tools.tar.gz"
printf '%s  %s\n' 446535cba9de418bd33e73fdb7e197bbfe25fcbb156fa8df9ca2714438aa48d7 "$TOOL_WORK/tools.tar.gz" | sha256sum --check --strict
mkdir "$TOOL_WORK/bin"
tar -xzf "$TOOL_WORK/tools.tar.gz" -C "$TOOL_WORK/bin"
export SURFPOOL_BIN="$TOOL_WORK/bin/surfpool"
export PLAYWRIGHT_ROOT="$TOOL_WORK/browser"
npm install --prefix "$PLAYWRIGHT_ROOT" --no-audit --no-fund playwright@1.56.1
"$PLAYWRIGHT_ROOT/node_modules/.bin/playwright" install --with-deps chromium
export TOOL_BROWSER_ONLY=1
unshare --net bash -ec 'ip link set lo up; exec node "$1/run.mjs"' -- "$scripts"
