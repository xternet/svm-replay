# Integrations

## Choose the execution route

| Route | Use |
|---|---|
| `surfpool start --after SIGNATURE_OR_SLOT` | Surfpool public RPC/WS, Studio, profiling and chain operations; SVM Replay historical VM/data |
| `anchor init my-project --test-template mocha --historical` | Test-owned Surfpool forks, with boundaries selected inside tests |
| `svm-replay fork --after SIGNATURE_OR_SLOT` | Standalone, limited RPC server for clients needing its supported methods only |
| `replay.replay({tx: SIGNATURE})` | Replay/verify one original transaction without starting a public chain server |

The first two require the integration builds and a matching installed SVM Replay
native bundle. They are not features of an unmodified upstream installation.
Neither route executes historical transactions in Surfpool's current LiteSVM.

## Provider setup

Export an Alchemy key with [Account Archive access](providers.md). Acquisition
can incur charges; a cached run may need no provider requests.

```sh
export API_ALCHEMY="YOUR_KEY"
surfpool start --after SIGNATURE --historical-provider alchemy --key env:API_ALCHEMY
```

Provider defaults belong in `Surfpool.toml`, or `Anchor.toml` for Anchor:

```toml
[historical]
provider = "alchemy"

[historical.providers.alchemy]
adapter = "alchemy"
key_env = "API_ALCHEMY"
# Optional: url = "https://solana-mainnet.g.alchemy.com/v2/{key}"
```

`alchemy` names an adapter, not the key. Keep credentials in shell/CI secrets,
not TOML, argv literals, fixtures or receipts. Missing inputs fail explicitly;
there is no current-mainnet data fallback.

## Historical forks (Surfpool, Anchor, CI)

Surfpool owns public RPC/WS, confirmations, local block history, cheatcodes,
profiling and Studio. A private SVM Replay backend owns historical reconstruction,
the pinned VM, execution, account state, clock and blockhash ages.

A signature selects state immediately after that transaction. A numeric slot
selects the next produced finalized slot, initialized before transaction zero;
it does not execute the block's first transaction. Existing `tx:`/`slot:`
prefixes remain accepted.

The fork starts frozen. Tab advances one slot; `surfnet_timeTravel` chooses an
explicit target. Crossing an epoch is a synthetic test warp: clock/epoch change,
but historical rewards, stakes and feature activations are not replayed.
`surfnet_resetNetwork` returns to the starting boundary, removing local changes.

```sh
surfpool start --after 413023260 --no-tui
surfpool stop --rpc-url http://127.0.0.1:8899
```

Do not use a historical datasource URL with a current-runtime Surfpool as a
substitute for this route. A datasource supplies accounts; it cannot replace
the executing VM.

### Anchor

Generate a workspace with the integration build:

```sh
anchor init my-project --test-template mocha --historical
cd my-project
anchor build --arch v0
anchor test --skip-build --skip-deploy
```

Use an SBF format supported at the chosen boundary; `v0` is the conservative
choice in this example. The generated `HISTORICAL.md` and helper show the full
test setup. Mocha/Jest support TS and JS; the RPC-based Rust template is separate.
In-process LiteSVM/Mollusk templates cannot use the historical backend.

Anchor compiles its ordinary fixtures into a launch plan. Each test or test hook
starts its own Surfpool child with the generated `HistoricalFork` helper,
chooses `after`, creates an explicit provider, and closes the fork afterwards.
There is no suite-wide historical `ANCHOR_PROVIDER_URL` before a fork exists.
Do not add `skip_local_validator = true`: the historical adapter owns startup.
`anchor localnet` is unchanged; use `surfpool start --after …` for an interactive
historical fork. Other CLI commands need an explicit running-fork endpoint.

The helper's fast program installation is not a loader deployment. A program
deployed/upgraded through the actual loader becomes callable after an explicit
slot advance. The helper can use normal Surfpool RPCs for local account edits,
reset and time travel; it does not implement another VM or RPC server.

Test funding and declared new accounts are explicit hypothetical changes, not
historical facts. Unknown historical accounts are reconstructed or refused.
Fixtures whose default rent differs from the boundary fail instead of silently
changing their bytes.

Transaction-v1 execution and fee queries require the installed worker's
`native-transaction-v1/v1` capability (currently Agave4.2). Older historical
workers return `UNSUPPORTED_TRANSACTION_VERSION`; use legacy/v0 transactions
for those boundaries. This is a runtime-format limit, not missing provider data.

Historical mode rejects legacy `test.validator.account`, `account_dir` and
`test.genesis` declarations rather than ignoring their supplied images.
Use the generated fund/mint/token fixtures, explicit `fork.setAccount()` /
`installProgram()` setup, or supported Surfpool snapshots.
`test.validator.clone` addresses hydrate lazily from the historical boundary;
they are not copied up front. Configure archive access in `[historical]`, not
the legacy `[test.validator] url`.

Closing a fork stops its owned child. If the test process dies, the still-running
Anchor command reaps registered forks. Killing Anchor itself with SIGKILL needs
job/cgroup cleanup; a JavaScript finally block cannot guarantee that case.

Application CI examples: [ordinary Anchor](../examples/ci/anchor-normal.yml),
[offline historical Anchor](../examples/ci/anchor-historical-offline.yml), and
[optional live-provider historical tests](../examples/ci/historical.yml).
Historical recipes need matching pinned integration tools; stock release hashes
are not substitutes. The offline recipe also needs a reviewed, redistribution-cleared
recorded fixture; no public fixture URL is supplied yet. See the
[runner and fixture requirements](../examples/ci/README.md).

## Standalone RPC: limited alternative

```sh
svm-replay fork --after 413023260 --port 8999 --provider alchemy --key env:API_ALCHEMY
```

This serves account reads, simulation, signed sends and its supported
confirmation methods. It is **not** Surfpool's full RPC/WS surface: account/log
subscriptions, chain-wide scans, Studio and Surfpool-specific APIs are not
provided by this server. Use Surfpool for ordinary Anchor workflows.

Standalone commits are immediately `finalized` within the local fork, with
`confirmations: null`; this is not mainnet consensus finality. Surfpool owns its
own confirmation policy when using the private backend.

The SDK `Replay.installed().forkAt({after, lockfile})` owns the same standalone
server. Its `timeoutMs` limits the whole fork lifetime, not just startup.
A lockfile pins boundary/runtime resolution; separate parallel tests need
separate locks. See [subprocess/SDK integration](tool-integration.md).

Standalone controls include `fork.setBalance()`, `overrideAccounts()`,
`snapshot()`, `select()`, `reset()`, `advance()` and `close()`.
Those VM branch snapshots are not Surfpool's chain-store persistence.
`svmReplay_advance [slots, {unixTimestamp, syntheticEpoch}]` is the RPC form;
cross-epoch advancement requires the explicit synthetic warp capability.

## Replay by signature

```ts
import { Replay } from "@xternet/svm-replay";
const replay = await Replay.installed();
const result = await replay.replay({ tx: "SIGNATURE", collect: { calls: true } });
console.log(result.response);
```

`installed()` trusts the selected local installation pin; `Replay.open()`
accepts an independently supplied bundle digest. Cancellation waits for owned
child cleanup. Replay does not broadcast a mainnet transaction.

`withHistoricalReplay(surfnet, replay)` is a non-mutating simulation companion;
it does not import a historical VM into an ordinary Surfpool.
`anchorSignature(signature, transaction)` encodes an already-built replacement
for replay. These companions are separate from full historical forks.

## Historical testing (experimental local integration)

Use [the Surfpool/Anchor route above](#historical-forks-surfpool-anchor-ci) for
normal developer tests and [the standalone guide](tool-integration.md) only for
its narrower interface. Runtime compatibility strata are not proof of the exact
validator build; newer slots are explicitly unqualified. Missing provider data
and unsupported operations remain typed errors, never invented empty results.

Assert the transaction's actual status and effects as well as `COMPLETED`.
A correctly simulated failed transaction can have a completed replay.
Keep redacted receipts/pins; never publish credentials or private account data.
[Limits](limitations.md) · [Examples](../examples/README.md) ·
[SDK](../src/bindings/typescript/README.md).
