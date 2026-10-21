# Interface tests

| Directory | Responsibility |
| --- | --- |
| cli/ | CLI coordinator, JSON formatting, debugger, cancellation and worker ownership |
| sdk/ | Installed SDK, traces, cancellation, concurrency, rejection and typings |
| integrations/ | Real Surfpool/Anchor consumers, historical forks (fork/) and an independent Rust host |
| packaging/ | Package metadata, bundles, read-only installation and retained-release evidence audit |
| layout/ | Repository structure, registered targets, links and imports |

Run the structural guard after every change:

```sh
node --test tests/interfaces/layout/repository.test.mjs
```

Rust target names remain unchanged; cargo test --workspace discovers them.
Historical consumer drivers are opt-in and explain their arguments when invoked
without them. The fork drivers (integrations/fork/): surfpool.mjs (served state and
each transaction's verified result), flows.mjs (the Surfpool user-flow matrix),
surfnet-sdk.mjs (Surfpool's Node SDK), anchor.mjs (anchor test), anchor-suites.mjs
(Anchor's own test projects against its default local Surfpool), anchor-idl.mjs
(Anchor's client with a mainnet program's IDL), anchor-branches.mjs (anchor localnet;
newer sBPF on older history), forks.mjs (several forks at once), battle.mjs (a new program,
installed over a mainnet program's code and composed with the original transaction, checked
exactly against SVM Replay; its program is battle/, built with cargo-build-sbf) and journey.mjs
(the CLI). Ordinary SDK unit tests live with the binding under
src/bindings/typescript/tests/.
