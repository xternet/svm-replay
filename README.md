# SVM Replay

[![crates.io](https://img.shields.io/crates/v/svm-replay.svg)](https://crates.io/crates/svm-replay)
[![npm](https://img.shields.io/npm/v/%40xternet%2Fsvm-replay.svg)](https://www.npmjs.com/package/@xternet/svm-replay)
[![GitHub release](https://img.shields.io/github/v/release/xternet/svm-replay)](https://github.com/xternet/svm-replay/releases/latest)
[![CI](https://github.com/xternet/svm-replay/actions/workflows/ci.yml/badge.svg?branch=main)](https://github.com/xternet/svm-replay/actions/workflows/ci.yml?query=branch%3Amain)
[![License: Apache-2.0](https://img.shields.io/badge/license-Apache--2.0-blue.svg)](LICENSE)

Replay a Solana transaction at its historical position, then test a different
payload, account state or program code.

- **Historical data:** Built on the historical state required for correct replay (not a rough estimate), using Alchemy's [Account Archive](https://www.alchemy.com/blog/solana-account-archive) (August 2026; history since July 2025).
- **Checked replay:** Rebuilds the state before your transaction and checks the original result before testing your changes.
- **Validation:** Tested on complex DeFi transactions; missing historical data returns explicit unsupported results. [Results](docs/validation.md) · [Limitations](docs/limitations.md).

## What to use it for

- **Debugging:** reproduce a failed transaction and test a fix.
- **Backtesting:** try alternative swaps, arbitrage or liquidation payloads.
- **Security:** investigate historical incidents and changed inputs.
- **Regression tests:** replay real cases in CI or from developer tools and agents.

## How to run

[Native downloads](https://github.com/xternet/svm-replay/releases/latest) ·
[Rust CLI](https://crates.io/crates/svm-replay) ·
[Rust API](https://crates.io/crates/svm-replay-engine) ·
[TypeScript SDK](https://www.npmjs.com/package/@xternet/svm-replay)

Cargo builds the CLI; npm provides the SDK and launcher. Both need the **native runtime bundle**.

Requirements:

- Installed `svm-replay` command: [installation guide](https://github.com/xternet/svm-replay/blob/main/docs/installation.md#guided-offline-installation).
- For live replay only: an Alchemy key with [Account Archive access](https://github.com/xternet/svm-replay/blob/main/docs/providers.md) and a transaction signature.
- Automatic runtime selection covers reviewed slots **353951234–451646918**, not every transaction in Alchemy's archive. Missing runtime/state support returns an explicit outcome.

Examples use Bash/Zsh; live data requests consume Alchemy credits.
Measured cold examples cost roughly **$0.00015–$0.019** at published RPC rates;
see [measurements and billing assumptions](docs/providers.md#measured-cold-examples).

### 1. Replay a cached example (offline demo)

```sh
svm-replay --demo
```

Replays bundled inputs to demonstrate the tool; no key or network.

### 2. Replay your transaction with a key

```sh
API_ALCHEMY="your-key" svm-replay --tx SIGNATURE
```

Replace `SIGNATURE` with your transaction signature; no wallet or broadcast.
Returns JSON: status, logs, CU, account changes and verification receipt.
Unsupported replays return a [category and available partial evidence](docs/usage.md#incomplete-replays), never an invented final state.
JSON is indented automatically in your terminal; pipes and saved files stay compact.
Inline keys may enter shell history; prefer `.env` below.

### 3. Keep your key in `.env`

Put in `.env` (keep private and out of Git):

```dotenv
API_ALCHEMY="your-key"
```

Then run (the CLI reads `.env` automatically):

```sh
svm-replay --tx SIGNATURE
```

Works on all platforms; use `.env` in the directory where you run the command.

### 4. Choose what to test

With your key in `.env`:

| Command | Purpose |
|---|---|
| **Replay** | |
| `svm-replay` | Interactive menu |
| `svm-replay --tx SIGNATURE` | Replay; print readable JSON automatically |
| **Output & saving** | |
| `svm-replay --tx SIGNATURE --json` | Force compact JSON |
| `svm-replay --tx SIGNATURE --human` | Readable summary |
| `svm-replay --tx SIGNATURE --out ./results` | Save compact JSON to a file |
| `svm-replay --tx SIGNATURE --pretty --out ./results` | Save indented JSON to a file |
| `svm-replay --tx SIGNATURE --fields status,logs,computeUnits` | Select output fields |
| **Account overrides & replacement** | |
| `svm-replay --tx SIGNATURE --overrides '[{"pubkey":"ACCOUNT_ADDRESS","lamports":"1000000000"}]'` | Set an account to 1 SOL before replaying the original |
| `svm-replay --tx SIGNATURE --overrides @./accounts.json` | Same override format, loaded from a JSON file |
| `svm-replay --tx SIGNATURE --replace '{"transactionBase64":"BASE64_SERIALIZED_TRANSACTION"}'` | Execute a new transaction instead of the original, at the same historical boundary |
| `svm-replay --tx SIGNATURE --replace @./payload.json` | Same replacement format, loaded from a JSON file |
| **Execution data** | |
| `svm-replay --tx SIGNATURE --collect '{"calls":true,"instructions":true,"registers":true,"memory":false}'` | Choose extra execution data |
| `svm-replay --tx SIGNATURE --collect @./capture.json` | Same collection format, loaded from a JSON file |
| `svm-replay --tx SIGNATURE --collect all` | Complete collection of all supported categories, or an explicit error |
| **Cache & help** | |
| `svm-replay --tx SIGNATURE --no-cache` | Run without reusable cache |
| `svm-replay doctor` | Check installation |
| `svm-replay --help` | All options |

Standard JSON includes status, logs, CU, account changes and verification.
Terminal JSON is indented by default. Add `--pretty` to indent piped or saved JSON too;
use `--json` to force compact terminal output. Data is unchanged.
Use `--human` for a short summary instead; it cannot be combined with `--pretty`.
`--fields` limits returned fields; it does not change what `--collect` captures.

`@path` reads a JSON file with the same structure as the inline example.
Replace capitalized placeholders; `transactionBase64` is a complete serialized
transaction encoded as base64, not a signature ([construction example](examples/anchor/README.md)).
Overrides/replacements start before the original transaction; unchanged-original
verification runs separately. Nothing is broadcast. Options can be combined:

```sh
svm-replay --tx SIGNATURE --replace @./payload.json --overrides @./accounts.json --collect all --out ./results
```

### Choose additional data

- **calls:** nested program calls, such as Aggregator → DEX → Token Program.
- **instructions / registers:** VM instruction positions and register values, captured together.
- **memory:** supported memory observations; also includes instruction/register data.

Omitted switches are false; dependencies above still apply. Capture retains call
context. Without explicit collection limits, incomplete capture is an error;
explicit limits allow labelled partial output. Unsupported requests fail explicitly;
truncated captures are labelled.

[Collection filters, limits and examples](https://github.com/xternet/svm-replay/blob/main/docs/tracing.md)
· [Payloads, overrides and other options](https://github.com/xternet/svm-replay/blob/main/docs/usage.md)

## Why this exists

Standard Solana RPC can return old transactions and blocks, but not arbitrary
historical account data. Fetching an old transaction and executing it against
current accounts does not reproduce its historical conditions.

Historical accounts alone are not enough either: replay needs the state immediately
before the target transaction, relevant predecessor effects and compatible runtime
rules. SVM Replay reconstructs that boundary and verifies the original before
testing modifications. Alchemy is the initial data provider; its archive excludes
vote accounts and per-slot sysvars. [Provider coverage and limitations](https://github.com/xternet/svm-replay/blob/main/docs/providers.md).

[Validation evidence](https://github.com/xternet/svm-replay/blob/main/docs/validation.md) ·
[Platform requirements](https://github.com/xternet/svm-replay/blob/main/docs/installation.md#platforms) ·
[Full limitations](https://github.com/xternet/svm-replay/blob/main/docs/limitations.md)

## Integrations and documentation

Use the CLI directly, the [TypeScript SDK](https://github.com/xternet/svm-replay/blob/main/src/bindings/typescript/README.md),
or the [Rust API](https://github.com/xternet/svm-replay/blob/main/examples/rust/README.md).
[Surfpool](https://github.com/xternet/svm-replay/blob/main/examples/surfpool/README.md),
[Anchor](https://github.com/xternet/svm-replay/blob/main/examples/anchor/README.md) and
[CI](https://github.com/xternet/svm-replay/blob/main/examples/ci/README.md) examples are our
integration recipes, not upstream features or endorsements.

[Usage/results](https://github.com/xternet/svm-replay/blob/main/docs/usage.md) ·
[Examples](https://github.com/xternet/svm-replay/blob/main/examples/README.md) ·
[Trace exports](https://github.com/xternet/svm-replay/blob/main/docs/tracing.md) ·
[Capabilities and interface gaps](https://github.com/xternet/svm-replay/blob/main/docs/capabilities.md) ·
[Architecture](https://github.com/xternet/svm-replay/blob/main/docs/architecture.md) ·
[Contributing](https://github.com/xternet/svm-replay/blob/main/CONTRIBUTING.md) ·
[Security](https://github.com/xternet/svm-replay/blob/main/SECURITY.md)

Owned code: [Apache-2.0](https://github.com/xternet/svm-replay/blob/main/LICENSE).
Runtime components retain their licenses; see [NOTICE](https://github.com/xternet/svm-replay/blob/main/NOTICE).
