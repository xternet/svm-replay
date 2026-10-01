# Clean Linux release checks

The Dockerfile prepares Ubuntu 24.04 with Node 22, native shared libraries and
the actual SDK tarball installed through npm. Its build context should contain
only the reviewed SDK tarball named sdk.tgz; never include provider keys or captures.
Image preparation uses the network. Replay tests must use --network none.

Build with Docker using this Dockerfile and that separate context. Record the
resulting image ID and run by that ID. Base images are digest-pinned; record the
installed package versions too, because apt/npm metadata can change.

Use the portable [installation check](../clean_host.mjs) inside the image with:

- an authorized prepared request and independently supplied SHA-256 pins;
- read-only mounts for the bundle, request and check script;
- a separate writable result directory owned by the container's non-root UID;
- --network none --read-only --cap-drop ALL --security-opt no-new-privileges;
- --cpus 4 --memory 8g --memory-swap 8g --pids-limit 256;
- a writable /tmp tmpfs for SDK temporary requests.

No development checkout, Cargo/npm cache, credentials or host network is needed
during execution. Retain stdout/stderr, summary.json, image ID and container
inspection showing the actual mounts, limits and network mode. Failed attempts
must not overwrite earlier evidence.

Containers validate clean Linux userspaces on the same kernel/CPU, not other
architectures or independent hardware. An older libc can reject at the ELF loader
before the application runs; this is not a typed engine outcome. Current workers
showed a GLIBC_2.38 loader requirement in the original negative check; supported
release installation requires glibc >= 2.39. These are not competing support
claims. Do not substitute newer host libraries in negative tests.

For trace checks, use a reviewed trace request with matching output/capture
budgets, not a smaller ordinary-replay request. An output-limit rejection must
remain visible; do not silently raise limits in the engine.

The historical benchmark index lists targets, not redistributable account-state
fixtures. Use your own authorized data; private provider snapshots are not shipped.
