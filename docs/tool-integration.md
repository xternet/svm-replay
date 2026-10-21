# Integrate historical forks

For a full developer node, use Surfpool's public RPC and WebSocket:
`surfpool start --after SIGNATURE_OR_SLOT`. Surfpool owns chain/RPC/Studio;
SVM Replay owns historical VM/data. Anchor uses this route through its generated
test helper; see [the Anchor guide](integrations.md#anchor).

The examples below use the **standalone, limited RPC server**, not Surfpool.
It supplies historical account reads, simulation and signed sends, but not the
full Surfpool surface or ordinary Anchor account/log subscription workflows.
No engine embedding or runtime selection is needed in the caller.

## Prerequisites

- Node.js 22+ and `npm install @xternet/svm-replay` for the SDK example.
- A matching [installed native bundle](installation.md), including its workers.
- An Alchemy key with [Account Archive access](providers.md), exported as
  `API_ALCHEMY`. Never commit it or pass its literal value on the command line.

## Node.js: own a fork

Save this as `historical.mjs` in your project:

```js
import assert from "node:assert/strict";
import { Replay } from "@xternet/svm-replay";

const [bundle, after] = process.argv.slice(2);
if (!bundle || !after) throw new Error("Usage: node historical.mjs BUNDLE SIGNATURE_OR_SLOT");

const replay = await Replay.installed(bundle);
const fork = await replay.forkAt({
  after,
  provider: "alchemy",
  key: "env:API_ALCHEMY",
  timeoutMs: 300_000, // Whole fork lifetime, not just startup.
}); // Resolves only when the fork is ready; startup failures reject.

try {
  console.log({ rpc: fork.url, websocket: fork.wsUrl, pins: fork.pins });
  const response = await fetch(fork.url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ jsonrpc: "2.0", id: 1, method: "getHealth", params: [] }),
    signal: AbortSignal.timeout(10_000),
  });
  if (!response.ok) throw new Error(`RPC HTTP ${response.status}`);
  const result = await response.json();
  if (result.error) throw new Error(JSON.stringify(result.error));
  assert.equal(result.result, "ok");

  // Connect your existing client to fork.url and run your tests here.
} finally {
  await fork.close(); // Also closes the owned native workers.
}
```

Run it with your bundle directory and historical boundary:

```sh
node historical.mjs ./svm-replay 413023260
```

For an activated user-level installation, use `Replay.installed()` without an
argument. No native bundle directory needs to be created in your project.

A signature selects state immediately after that transaction. A slot selects
the initialized next produced slot, before its transactions execute.
Requests can fetch historical data and consume provider credit; later reads use
loaded state or the cache. Unavailable inputs fail instead of using current state.

For reproducible tests, add `lockfile: "historical.lock.json"` to `forkAt` and
commit the generated file. Use separate files for independently initialized
parallel tests. A changed boundary/runtime resolution fails rather than silently
updating the lock. See [SDK options](../src/bindings/typescript/README.md).

## Any language: connect over RPC

```sh
svm-replay fork --after 413023260 --port 8999 --provider alchemy --key env:API_ALCHEMY
```

HTTP: `http://127.0.0.1:8999`; WebSocket: `ws://127.0.0.1:9000`.
Use normal Solana clients for account reads, simulation, signed sends and
confirmation. Your program must match the historical runtime's supported format.

For subprocess integration, add `--json`: stdout emits the structured `kind: ready`
handshake with `url`, `wsUrl` and `pins`; stderr carries diagnostics. Do not parse
human output. Handle early exit, malformed output and startup deadlines; stop
your owned server with `svmReplay_close`, and reap the child. Never stop another
tool's fork or forward provider credentials to the local RPC endpoint.

## Fork controls

| SDK | Purpose |
|---|---|
| `fork.setBalance(address, 1_000_000_000n)` | Explicit local test funding |
| `fork.overrideAccounts([{pubkey: address, program: "@my_program.so"}])` | Load a local program |
| `fork.snapshot()` / `fork.select(id)` | Save/select a test branch |
| `fork.reset()` | Remove local changes and restore the initial boundary |
| `fork.advance(1)` | Advance one local slot explicitly |
| `fork.close()` | Shut down the owned server/workers |

These have `svmReplay_*` RPC equivalents; see [RPC and testing details](integrations.md#historical-testing-experimental-local-integration).
Funding and overrides are hypothetical local edits, never historical facts.
Declare new accounts explicitly rather than assuming an unknown address is empty.

## Compatibility checklist

- Slots/time are frozen. Loader-deployed/upgraded programs become callable after
  explicit advancement; advancement does not execute later archived transactions.
- Advancement stays within the boundary's epoch, except an explicit synthetic test
  warp (the Clock only; no epoch transition). This is not a full validator or
  a general-purpose mainnet RPC proxy; unsupported methods return errors.
- Missing provider data, unsupported runtimes and verification failures must stay
  visible. Never retry against current mainnet state as a fallback.
- WebSocket supports signature/slot subscriptions, not every subscription API.
- Connect only local clients: the fork RPC is loopback-only and permits test edits.

Check [limitations](limitations.md) before advertising compatibility. For concrete
tool integrations, see [Surfpool and Anchor](../README.md#surfpool).
