# Local installation

For version 0.1.0, select a native bundle from the
[GitHub release](https://github.com/xternet/svm-replay/releases/tag/v0.1.0)
matching your OS and architecture; [qualification status](validation.md) is separate
from implementation support. Worker binaries are reviewed/pinned separately from the coordinator.
Rust callers can use the published crates or workspace path dependencies. Owned packages
include the Apache-2.0 license and project notice.

## Packages

| Package | Purpose |
|---|---|
| [svm-replay](https://crates.io/crates/svm-replay) | Rust CLI; Cargo builds the coordinator, not runtime workers |
| [svm-replay-engine](https://crates.io/crates/svm-replay-engine) | Rust API; Cargo resolves its protocol/store dependencies |
| [@xternet/svm-replay](https://www.npmjs.com/package/@xternet/svm-replay) | TypeScript SDK and CLI launcher; native bundle required |

The npm 0.1.0 submission is awaiting registry review. Its `0.0.0-stage` version
is a nonfunctional placeholder; use the GitHub release's SDK archive meanwhile.

The CLI crate contains the executable source and Rust example. The complete test
suite, other packages and integration examples live in the repository checkout;
development dependencies and generated SDK files are not shipped in the CLI crate.

Choose one path:

- **Try the tool:** install a prebuilt native archive and run its offline demo.
- **Use JavaScript/TypeScript:** install the SDK as well; it calls the same native CLI.
- **Build from source:** Cargo builds the coordinator, not the patched runtime workers.

Staged native archives include `runtime-sources/` (fourteen reference/capture builds, locks,
build/check script, patch records and dependency notices). Use that packet to
rebuild workers; a catalog alone is not their source code. Rebuilt worker identities
need qualification before being used as a reviewed release.

## Platforms

See the guided installer and offline demo below; the following requirements apply
to both manual and guided installation.

- Linux x86_64/ARM64: GNU libc >= 2.39 and system SQLite. Alpine/musl and older
  glibc builds are not qualified. WSL2 uses the Linux bundle, not the Windows one.
- macOS Intel/Apple Silicon: native bundles use system libraries. Qualification
  targets macOS 15; older versions are not claimed as tested.
- Windows x64/ARM64: install the matching current
  [Microsoft Visual C++ v14 Redistributable](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist).
  SQLite is compiled into the coordinator. Preserve the normal `SystemRoot`
  environment variable; workers need it for debugger loopback networking.
  Qualification used Windows Server 2025 x64 and Windows 11 ARM64.

The SDK needs Node 22+ and the same native bundle. Windows executables end in
`.exe`; use Windows paths and PowerShell quoting instead of the shell examples
below. No emulation, nearest-runtime substitution or automatic worker download.

The native candidates are not developer-signed or notarized. OS download
protections may request approval; verify the release source and trusted checksums
before allowing execution. Do not disable system-wide protections to install them.

On Linux, runtime shared libraries are not bundled: the CLI needs libsqlite3.so.0,
libgcc_s.so.1 and the standard GNU C/math loader libraries; workers need the latter
GNU libraries. On Ubuntu 24.04 these are supplied by libsqlite3-0, libgcc-s1 and libc6.
Building additionally needs Rust 1.94.0, pkg-config and libsqlite3-dev. Clean-container
qualification covers Ubuntu 24.04 (glibc 2.39) and Debian 13 (glibc 2.41).
These environments share the test host's kernel and CPU; they are not independent hardware tests.

## CLI and workers

### Guided offline installation

After publication, download `release.json` and the native archive for your OS/CPU
from the same release. Obtain the **release.json SHA-256 independently** from
trusted release checksums. Put the archive beside the manifest. With Node 22+
and `tar` installed:

```sh
npx --package @xternet/svm-replay@0.1.0 svm-replay-install \
  --release ./release.json --sha256 "$RELEASE_SHA256" --output ./svm-replay
```

The helper selects the native OS/CPU entry, verifies the archive before extraction,
rejects links/unsafe paths, runs the bundle doctor and installs into a **new**
directory. It preserves source/license notices and changes no PATH or system
settings. `npx` may fetch the npm package; the installer itself is offline.
PowerShell:

```powershell
npx --package @xternet/svm-replay@0.1.0 svm-replay-install --release .\release.json --sha256 $env:RELEASE_SHA256 --output .\svm-replay
.\svm-replay\bin\svm-replay.exe demo
```

For an already downloaded/extracted npm package, run `node package/install.mjs`
with the same options. No registry login is needed to consume a public release.

The executable is `svm-replay/bin/svm-replay` (`.exe` on Windows).
Run without arguments for the terminal menu. This path is relative to the installation
destination, not the source checkout. Add its `bin` directory to PATH yourself to
use the short `svm-replay` commands in these guides.
`installation.json` retains the locally trusted bundle pin; protect installation
files against modification. That local record is not a digital signature.

To get the short command through npm (after publication):

```sh
npm install -g @xternet/svm-replay@0.1.0
svm-replay-install --release ./release.json --sha256 "$RELEASE_SHA256" --output ./svm-replay --activate
svm-replay doctor
svm-replay --demo
```

`--activate` selects the verified installation in a small OS application-data
record. npm supplies the executable wrapper; its global bin directory must be on
PATH. No shell profiles or system PATH settings are edited. Without activation,
the original `./svm-replay/bin/svm-replay` path remains usable. `doctor` and
`capabilities` automatically verify the running installation's manifest pin.

### Cached offline demo

For an installation containing an approved demo dataset:

```sh
./svm-replay/bin/svm-replay demo
```

The command verifies the bundled request, executes it afresh with result caching
off, compares the expected output and prints the receipt location. No wallet,
API key or network is needed. This demonstrates **supplied-state replay**, not
fresh archive acquisition. Replay success is separate from transaction success.

The release contains demo data; the owner approved inclusion.
No provider endorsement or separate
permission is implied. The Git repository contains no provider snapshots.
Bundles without a demo reject explicitly and never download data silently. Demo
inputs are immutable installation files, separate from disposable result caches.

The terminal demo shows a menu, progress and readable results. `--example default`
skips selection; `--json` forces compact JSON. Terminal JSON otherwise uses indentation;
redirected output stays compact unless `--pretty` is specified. Progress appears only on
terminal stderr in JSON mode; captured/CI runs stay quiet. See [usage](usage.md).

### Manual bundle preparation

Build with Rust 1.94.0: `cargo build --release --locked`. Given an already reviewed
worker catalog and an independently obtained SHA-256:

```sh
./target/release/svm-replay bundle pack \
  --catalog /absolute/path/catalog.json --catalog-sha256 "$CATALOG_SHA256" \
  --output /absolute/path/new-bundle
```

Optional `--family v3-0` (repeat or comma-separate) includes only selected installed
families. `--reference-only` omits trace/debug workers; those modes then reject
explicitly. No closest-version fallback. The current Linux seven-family runtime bundle
is about 1.88 GB uncompressed; its archive, including worker sources and notices,
is about 488 MB. Other platform sizes depend on their builds.

`pack` prints a bundle SHA-256. Retain/distribute that pin through a trusted separate
channel; hashing an untrusted download by itself does not establish authenticity.
The directory contains `bundle.json`, `catalog.json`, `bin/svm-replay`, `workers/`.
No extraction or automatic download is involved.

```sh
./target/release/svm-replay bundle install \
  --bundle /absolute/path/new-bundle/bundle.json --bundle-sha256 "$BUNDLE_SHA256" \
  --output /absolute/path/new-install
/absolute/path/new-install/bin/svm-replay doctor \
  --bundle /absolute/path/new-install/bundle.json --bundle-sha256 "$BUNDLE_SHA256"
```

The destination must not exist. Installs verify confined regular files, all hashes,
catalog identities and host compatibility. Files copy before the manifest; failed
partial directories remain for diagnosis, never silently resumed or overwritten.
Keep installations private/non-writable by other users. This is integrity checking,
not a sandbox against a malicious local user who controls your trusted directory.

Use the included CLI with `simulate --bundle ... --bundle-sha256 ...`. Do not mix
it with a different coordinator. State goes to OS application data (or `--data-dir`),
not the installation. Nothing writes to the repository by default.

## TypeScript / JavaScript

```sh
cd src/bindings/typescript
npm ci --ignore-scripts
npm test
npm pack
# In a separate consumer project:
npm install /absolute/path/xternet-svm-replay-0.1.0.tgz
```

The tarball includes compiled ESM and declarations, a Node type-only dependency, no runtime dependencies or
postinstall scripts. Node 22 and Bun are tested callers; both still need qualified
native workers. `Replay.open({bundlePath, bundleSha256, ...})` verifies the install.
See the [SDK guide](../src/bindings/typescript/README.md) and [integration examples](integrations.md).

The planned package is `@xternet/svm-replay@0.1.0`. No PyPI package
is claimed; other languages can call the JSON CLI and retain its receipts.

## Clean-environment installation check

In a clean Linux x86_64 container or separate host with the libraries above and Node 22, copy the
reviewed native bundle, an authorized prepared request and
[clean_host.mjs](../tests/interfaces/packaging/clean_host.mjs). Obtain their expected
pins and the reference output pin through a trusted separate channel, then run:

```sh
node clean_host.mjs /path/to/bundle/bundle.json BUNDLE_SHA256 \
  /path/to/request.json REQUEST_SHA256 EXPECTED_OUTPUT_SHA256 \
  /path/to/new-check-directory
```

This installs into a new directory, runs doctor and a real replay, checks the
original control, output hash and absence of current-state fallback, and saves
logs plus summary.json. No provider key or development checkout is required.
Failed attempts are preserved; use a new directory for another attempt.
This tests installation and one prepared case, not live fetching, tracing, the
whole corpus or independent consensus correctness. Running it on the development
machine does not constitute independent-host qualification. Clean-container checks
are the release's userspace qualification; a separate machine adds confidence but
is not claimed or required by this check. See the
[container recipe](../tests/interfaces/packaging/containers/README.md).
