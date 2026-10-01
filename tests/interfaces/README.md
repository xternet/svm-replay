# Interface tests

| Directory | Responsibility |
| --- | --- |
| cli/ | CLI coordinator, JSON formatting, debugger, cancellation and worker ownership |
| sdk/ | Installed SDK, traces, cancellation, concurrency, rejection and typings |
| integrations/ | Real Surfpool/Anchor consumers and an independent Rust host |
| packaging/ | Package metadata, bundles, read-only installation and retained-release evidence audit |
| layout/ | Repository structure, registered targets, links and imports |

Run the structural guard after every change:

```sh
node --test tests/interfaces/layout/repository.test.mjs
```

Rust target names remain unchanged; cargo test --workspace discovers them.
Historical consumer drivers are opt-in and explain their arguments when invoked
without them. Ordinary SDK unit tests live with the binding under
src/bindings/typescript/tests/.
