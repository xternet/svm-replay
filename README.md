# SVM Replay

[![crates.io](https://img.shields.io/crates/v/svm-replay.svg)](https://crates.io/crates/svm-replay)
[![npm](https://img.shields.io/npm/v/%40xternet%2Fsvm-replay.svg)](https://www.npmjs.com/package/@xternet/svm-replay)
[![GitHub release](https://img.shields.io/github/v/release/xternet/svm-replay)](https://github.com/xternet/svm-replay/releases/latest)
[![CI](https://github.com/xternet/svm-replay/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/xternet/svm-replay/actions/workflows/ci.yml?query=branch%3Amain)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

Replay Solana transactions, test changes, and fork historical state.

![#debugging](https://img.shields.io/badge/%23debugging-0F766E?style=flat-square) ![#simulation](https://img.shields.io/badge/%23simulation-0F766E?style=flat-square) ![#security-research](https://img.shields.io/badge/%23security--research-0F766E?style=flat-square) ![#backtesting](https://img.shields.io/badge/%23backtesting-0F766E?style=flat-square) ![#historical-testing](https://img.shields.io/badge/%23historical--testing-0F766E?style=flat-square)

## Quick start

Linux / macOS / WSL2:

```sh
curl -fsSL https://raw.githubusercontent.com/xternet/svm-replay/v0.2.0/install.sh | sh
export PATH="$HOME/.local/bin:$PATH"
svm-replay demo
```

Windows (PowerShell):

```powershell
& ([scriptblock]::Create((Invoke-WebRequest -UseBasicParsing https://raw.githubusercontent.com/xternet/svm-replay/v0.2.0/install.ps1).Content))
$env:PATH = "$env:LOCALAPPDATA\svm-replay\bin;$env:PATH"
svm-replay demo
```

No Node.js or Rust compiler required. Offline demo, no key. [Installation](docs/installation.md).

<sub>SDKs: <code>npm install @xternet/svm-replay@0.2.0</code> · <code>cargo add svm-replay-engine@0.2.0</code> (native bundle required).</sub>

## Setup

1. Install the `svm-replay` command and its native bundle: [installation guide](docs/installation.md).
2. For your own transactions, put an Alchemy key with
   [Account Archive access](docs/providers.md) in a `.env` file where you run it (keep it private):

   ```dotenv
   API_ALCHEMY="your-key"
   ```

A transaction's first replay downloads its history (measured $0.00015–$0.019 of Alchemy
credit, [details](docs/providers.md#measured-cold-examples)); later runs use the cache.

## Commands

<table>
<thead>
<tr><th align="left">Command</th><th align="left">Optional parameters</th><th align="left">Description</th></tr>
</thead>
<tbody>
<tr><td rowspan="10" valign="top"><code>svm-replay tx SIGNATURE</code></td><td>—</td><td>Replay and verify an on-chain transaction</td></tr>
<tr><td><code>--state-overrides @state.json</code></td><td>Modify starting balances, data or programs</td></tr>
<tr><td><code>--provider alchemy --key env:API_ALCHEMY</code></td><td>Select Alchemy and its credential</td></tr>
<tr><td><code>--config FILE</code></td><td>Load provider settings from TOML</td></tr>
<tr><td><code>--data-dir DIR</code></td><td>Choose the cache and run directory</td></tr>
<tr><td><code>--human</code></td><td>Print a readable summary</td></tr>
<tr><td><code>--collect all</code></td><td>Capture calls, instructions, registers and memory</td></tr>
<tr><td><code>--fields status,logs,computeUnits</code></td><td>Select output fields</td></tr>
<tr><td><code>--out DIR</code></td><td>Save results</td></tr>
<tr><td><code>--progress</code></td><td>Emit progress events on stderr</td></tr>
<tr><td rowspan="2" valign="top"><code>svm-replay payload PAYLOAD --after &lt;SIGNATURE|SLOT&gt;</code></td><td>—</td><td>Execute one transaction or an ordered bundle (JSON/@FILE)</td></tr>
<tr><td><code>--state-overrides @state.json</code></td><td>Modify starting account/program state</td></tr>
<tr><td rowspan="2" valign="top"><code>svm-replay fork --after &lt;SIGNATURE|SLOT&gt;</code></td><td>—</td><td>Serve a frozen historical RPC fork</td></tr>
<tr><td><code>--port 8999</code></td><td>Set HTTP port; WebSocket uses the next port</td></tr>
<tr><td><code>svm-replay demo</code></td><td>—</td><td>Run the offline example; no key needed</td></tr>
<tr><td><code>svm-replay doctor</code></td><td>—</td><td>Verify the installation</td></tr>
<tr><td><code>svm-replay runtimes</code></td><td>—</td><td>List installed runtimes and coverage</td></tr>
<tr><td><code>svm-replay completions SHELL</code></td><td>—</td><td>Generate shell completions</td></tr>
<tr><td><code>svm-replay --help</code></td><td>—</td><td>Show all commands and options</td></tr>
</tbody>
</table>

## Surfpool

These historical features require the integration build, not an unmodified upstream release.
Install a matching SVM Replay bundle and make its CLI available on `PATH`. Historical mode
launches SVM Replay's pinned historical runtime; it does not execute in Surfpool's current SVM.

Declare provider defaults in `Surfpool.toml`, keeping the actual key out of the file:

```toml
[historical]
provider = "alchemy"

[historical.providers.alchemy]
adapter = "alchemy"
key_env = "API_ALCHEMY"
```

Set `API_ALCHEMY` in your shell or CI secret environment. Then one command prepares and
serves the historical fork; no separate SVM Replay process needs to be started manually.

| Command | Description |
|---|---|
| `surfpool start --after SIGNATURE` | Start after the transaction, using `Surfpool.toml` provider defaults |
| `surfpool start --after SLOT` | Start at the initialized next produced slot |
| `surfpool start --after SIGNATURE --historical-provider alchemy --key env:API_ALCHEMY` | Select the provider and credential reference explicitly, without TOML |
| `surfpool stop --rpc-url http://127.0.0.1:8899` | Stop the historical RPC server |

HTTP defaults to `127.0.0.1:8899`; WebSocket uses port 8900. Surfpool owns the public RPC,
Studio and profiling; SVM Replay executes them on a private backend. Slot production is off
until explicit advancement. Surfpool keeps its normal TUI; new transactions show their
signatures, status and measured CU. Tab advances one slot; use `--no-tui` for headless operation.
Historical mode uses neither a current-state fallback nor `--match-datasource`.

## Anchor

Requires the integration builds of Anchor and Surfpool, plus an installed SVM
Replay native bundle. Generate a workspace:

```sh
anchor init my-project --test-template mocha --historical
cd my-project
anchor build --arch v0
anchor test --skip-build --skip-deploy
```

Set the provider in `Anchor.toml` and export `API_ALCHEMY`:

```toml
[historical]
provider = "alchemy"

[historical.providers.alchemy]
adapter = "alchemy"
key_env = "API_ALCHEMY"
```

Select the boundary **in test code**, using the generated helper:
The generated test's `before` hook funds the wallet, installs the ELF and declares
new accounts; keep that setup when using this abbreviated snippet.

```ts
const fork = await HistoricalFork.start({ after: "413023260" });
try {
  const program = new Program(idl, fork.provider());
  // Call program.methods.<method>(...).rpc() and assert the results.
  await fork.advance(1); // Time stays frozen unless explicitly advanced.
} finally {
  await fork.close();
}
```

| Command or test API | Description |
|---|---|
| `anchor init NAME --test-template mocha --historical` | Generate test-owned historical forks |
| `anchor build --arch v0` | Build a program format supported at the selected boundary |
| `anchor test --skip-build --skip-deploy` | Run the built program; each test starts its own fork |
| `HistoricalFork.start({after: SIGNATURE_OR_SLOT})` | Start an owned Surfpool child at that boundary |
| `fork.provider()` | Create an explicit Anchor provider for this fork |
| `fork.rpc(method, params)` | Use Surfpool's public RPC/cheatcodes |
| `fork.advance(1)` / `fork.close()` | Advance explicitly / stop the owned fork |

Mocha/Jest support TS and JS; the RPC Rust template is also supported.
In-process LiteSVM/Mollusk templates reject historical mode.
Anchor reuses its fixture compiler; Surfpool owns RPC/WS and chain operations;
SVM Replay owns historical data and execution. No SVM Replay directory or SDK
dependency is needed inside the workspace. The fast ELF fixture setup is not
a loader deployment.

See the generated `HISTORICAL.md`,
[integration details](docs/integrations.md#anchor) and
[CI example](examples/ci/README.md).

## Integrate into your own tool

Add historical forks and transaction replay to your tool through the SDK or RPC.
See the [integration guide](docs/tool-integration.md) for examples, lifecycle handling and limitations.

## Documentation

- [Why SVM Replay?](docs/why-svm-replay.md)
- [Installation](docs/installation.md)
- [Usage and results](docs/usage.md)
- [Integrate your tool](docs/tool-integration.md)
- [Forks](docs/integrations.md#historical-forks-surfpool-anchor-ci)
- [Traces](docs/tracing.md)
- SDKs: [TypeScript](src/bindings/typescript/README.md) · [Rust](examples/rust/README.md)
- [Examples](examples/README.md)
- [Capabilities](docs/capabilities.md)
- [Runtimes](docs/runtimes.md)
- [Validation](docs/validation.md)
- [Limitations](docs/limitations.md)
- [Architecture](docs/architecture.md)
- [Contributing](CONTRIBUTING.md)
- [Security](SECURITY.md)

Owned code: [Apache-2.0](LICENSE). Runtime components keep their licenses; see [NOTICE](NOTICE).
