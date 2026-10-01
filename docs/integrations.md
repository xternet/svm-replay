# Integrations

## Replay by signature

The SDK uses the same native signature workflow, including `API_ALCHEMY` or a
current-directory `.env`. No separate historical reconstruction code is required:

```ts
import { Replay } from "@xternet/svm-replay";
const replay = await Replay.installed("./svm-replay");
const result = await replay.replay({ tx: "SIGNATURE", collect: {calls: true} });
console.log(result.response); // selected result, traces and verification
```

`installed()` trusts your existing local installation pin. `Replay.open()` still
accepts an independently supplied bundle digest.
Both signature and prepared calls use that selected bundle/pin; the explicit
`Replay.open()` route does not require an adjacent installation.json.
`replay()` accepts replacement, override, field, cache, limit and pinned recovery-source controls; AbortSignal
waits for owned-child cleanup. It does not broadcast a transaction.
Use `collect: "all"` or a collection object for additional observations; `fields`
only selects returned data. Advanced `simulate()` retains its separate `trace`
configuration. See [collection settings](tracing.md#collection-settings).

The Surfpool companion accepts `simulateTransactionAt({tx: "SIGNATURE"})`.
For Anchor-built replacements, pass `anchorSignature(signature, transaction)`
to `replay.replay()`. Existing prepared-request methods remain available.

All consumers use the same pinned native engine. They do not supply a current-state
Bank or reconstruct historical state themselves. No integration requires an upstream
merge; these are our companions/examples, not endorsed upstream features.

## Surfpool

`withHistoricalReplay(surfnet, replay)` adds `simulateTransactionAt(options)` to a
non-mutating wrapper. Existing Surfpool methods retain their receiver. Local Surfpool
can continue serving ordinary development RPC; historical requests go to SVM Replay.
The example starts an owned offline Surfpool instance and stops it in `finally`.
It does not import the historical boundary into Surfpool or replace its SVM version.

## Anchor

`anchorSignature(signature, transaction)` serializes an already built replacement
for automatic reconstruction. `anchorRequest(historicalInput, transaction)` retains
the advanced prepared-input route. Build with explicit IDL, `.accountsStrict()`, historical
payer and blockhash. Do not call `.rpc()`, discover accounts through current-state RPC,
read a wallet or fetch the latest blockhash. The example uses Anchor's System transfer
encoding, then simulates the unsigned replacement at the historical boundary.
Signature verification/broadcast are not part of this counterfactual simulation.

## CI and other callers

`assertCompleted()` fails on NEEDS_INPUT/UNSUPPORTED/MISMATCH/etc. Then assert the
specific transaction status and expected effects; COMPLETED can contain a correctly
simulated failing transaction. Retain redacted receipts/pins as evidence. Never upload
provider keys or private payloads. The CI template runs offline unit/protocol tests;
historical CI additionally needs your explicitly pinned workers and captured evidence.

Rust embeds src/engine/mod.rs. Other languages can execute the JSON CLI; an HTTP server,
automatic issue submission and public language registries are not implied.
The retained debugger JSONL backend is experimental, not a release integration.
The TS SDK handles simulation/trace requests and file-backed results.

## Run examples

See [examples/README.md](../examples/README.md). Pinned test consumers:
`@solana/surfpool` 1.5.0, `@anchor-lang/core` 1.1.2, `@solana/web3.js` 1.98.4.
Upstream API changes should affect only companions/examples, not historical logic.
