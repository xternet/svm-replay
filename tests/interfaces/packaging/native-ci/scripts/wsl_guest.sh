#!/usr/bin/env bash
# Only the downloaded Linux candidate is installed; no Rust/runtime build here.
set -euo pipefail
candidate_commit="$1"
downloaded="$2"
host_reports="$3"
case "$candidate_commit" in *[!0-9a-f]*|'') exit 2;; esac
test "${#candidate_commit}" -eq 40
job_root=$(mktemp -d /tmp/svm-replay-wsl.XXXXXXXX)
mkdir "$job_root/reports"
copy_reports() {
    local status=$?
    for file in "$job_root/reports/"*.log "$job_root/reports/"*summary.json; do
        if [ -f "$file" ]; then cp -- "$file" "$host_reports/"; fi
    done
    exit "$status"
}
trap copy_reports EXIT
export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get install -y --no-install-recommends ca-certificates curl git python3 xz-utils
node_archive=node-v22.22.0-linux-x64.tar.xz
curl --fail --silent --show-error --location --max-time 180 \
    "https://nodejs.org/dist/v22.22.0/$node_archive" -o "$job_root/$node_archive"
# Published https://nodejs.org/dist/v22.22.0/SHASUMS256.txt, verified before pinning.
printf '%s  %s\n' 9aa8e9d2298ab68c600bd6fb86a6c13bce11a4eca1ba9b39d79fa021755d7c37 \
    "$job_root/$node_archive" | sha256sum --check --strict
mkdir "$job_root/node"
tar -xJf "$job_root/$node_archive" --strip-components=1 -C "$job_root/node"
export PATH="$job_root/node/bin:$PATH"
test "$(node --version)" = v22.22.0
git clone --no-checkout --depth 1 --branch 0.2.0 \
    https://github.com/xternet/svm-replay.git "$job_root/source"
git -C "$job_root/source" fetch --depth 1 origin "$candidate_commit"
git -C "$job_root/source" checkout --detach FETCH_HEAD
test "$(git -C "$job_root/source" rev-parse HEAD)" = "$candidate_commit"
cp -R -- "$downloaded" "$job_root/candidate"
export GITHUB_SHA="$candidate_commit"
export RUNNER_ARCH=X64
export PYTHONDONTWRITEBYTECODE=1
python3 "$job_root/source/tests/interfaces/packaging/native-ci/scripts/fresh_consumer.py" \
    "$job_root/source" "$job_root/candidate" "$job_root/reports"
