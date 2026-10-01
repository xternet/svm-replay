# Tests

Run from the repository root:

```sh
node --test tests/interfaces/layout/repository.test.mjs
node --test tests/interfaces/packaging/*.test.mjs
cargo test --workspace --locked --offline
cargo fmt --all --check
npm --prefix src/bindings/typescript test
npm --prefix examples test
npm --prefix tests ci --ignore-scripts
bun test tests
```

Install dependencies first on a fresh checkout. These ordinary checks do not
contact Alchemy. Lossless JSON is a declared test dependency, not imported from
a private prototype installation.

| Directory | Checks |
| --- | --- |
| harness/ | Shared setup, frozen policy, evidence and protocol fixtures |
| historical/ | Prepared corpus and source reconstruction |
| overrides/ | Transaction and account/program modifications |
| cache/ | Cold, prepared and result-cache reuse |
| tracing/ | Reference/capture parity and bounded exports |
| debugging/ | Live commands, runtime families, cleanup and receipts |
| [interfaces/](interfaces/README.md) | CLI, SDK, integrations, packaging and layout |

Component tests live with src/engine, src/protocol and src/store.
Cargo target names are stable and explicitly registered.

## Historical checks are opt-in

Historical drivers require reviewed, hash-pinned workers and retained datasets.
SVM_REPLAY_TEST_ARTIFACTS selects the retained corpus root. Some migration-only
drivers additionally require SVM_REPLAY_TEST_PROTOTYPE or SVM_REPLAY_PROTOTYPE;
those inputs are not bundled with this release candidate. No machine defaults are
used. Rust opt-in tests describe their other SVM_REPLAY_TEST_* inputs.

Drivers referring to retained prototype manifests or historical milestone schemas
are migration-evidence checks, not the public demo or a generic test-data downloader.
Their original schema IDs and expected hashes must remain unchanged.

Do not enable every ignored Rust test together: some are process-crash helpers
or separately authorized provider acquisitions. Read each test and opt in by its
exact name. Never alter stored expectations to make a mismatch pass.

The layout guard is offline and mandatory after every update. Installed SDK and
integration drivers execute supplied real historical inputs. Synthetic package
tests prove packaging behavior, not historical correctness.

For declaration isolation, copy interfaces/sdk/typescript-consumer.ts outside
the workspace, install the SDK tarball there and run tsc --noEmit --strict
--module NodeNext --target ES2022. Do not rely on ancestor node_modules.

See [validation and remaining release gates](../docs/validation.md).
