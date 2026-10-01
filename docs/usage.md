# Usage

Build: cargo build --release. Executable: target/release/svm-replay.
Use --help for commands. This build does not automatically install workers.
See [offline installation](installation.md) for pinned bundles and the local npm package.

## Signature workflow

Run the installed bundle, not a bare source-built binary without workers:

```sh
export API_ALCHEMY="your-key"
svm-replay --tx SIGNATURE
svm-replay --tx SIGNATURE --json
svm-replay --tx SIGNATURE --pretty --out ./results
svm-replay --tx SIGNATURE --replace @payload.json --overrides @accounts.json --collect '{"calls":true}'
svm-replay --tx SIGNATURE --out results --no-cache
svm-replay --tx SIGNATURE --fields status,logs,computeUnits
svm-replay --tx SIGNATURE --alchemy-key-file /path/to/key.txt --human
svm-replay --demo
```

PowerShell: `$env:API_ALCHEMY = "your-key"`. A key file contains only the key.
Alternatively, put `API_ALCHEMY="your-key"` in `.env` in the directory where you
run the command; the signature CLI and menu load it automatically on all platforms.
An explicit key file takes precedence over the environment, which takes precedence
over `.env`. Only API_ALCHEMY is read; other variables are not imported. Quoted or
unquoted keys, comments and optional `export` are supported, but shell expressions
and variable expansion are not. Invalid or duplicate keys fail without exposing
the value. Parent directories are not searched; `.env` is limited to 64 KiB.
Do not commit keys or put them in request JSON. The provider is never selected
silently when no key is configured. The offline demo needs no credential.

The CLI looks up the signature, resolves its exact transaction index, selects a
reviewed compatibility profile, reconstructs historical accounts and Bank inputs,
and verifies the original before executing any requested changes. No transaction
is broadcast. Reviewed runtime coverage ends at slot 451646918; profiles are not proof
that a unique validator binary executed a slot. Original matching is evidence,
not a consensus proof of every hypothetical replacement.
CU/meter-log differences are disclosed as metadata warnings; transaction status,
account effects and other semantic comparisons are not relaxed.
Changed meter lines must reconcile monotonically with the transaction's total CU
difference, including differences accumulated across sequential calls. Opposing,
disappearing or unreconciled meter differences still fail verification.

### Output formats

| Option | Output |
|---|---|
| No formatting flag | Indented JSON in a terminal; compact when piped or redirected |
| `--json` | Force compact JSON and disable menus |
| `--out ./results` | Compact JSON file; terminal acknowledgement follows terminal formatting |
| `--pretty` | Force indented JSON, including pipes and saved files |
| `--pretty --out ./results` | Indented JSON file and a terminal acknowledgement with its path |
| `--human` | Short readable summary, not JSON |

`--pretty` also works with `--demo`, `--collect`, `--fields` and `--json`.
With both `--json` and `--pretty`, indentation wins. JSONL debugger output always
stays one compact JSON object per line. Interactive menus still use human summaries.
It cannot be combined with `--human` or experimental `simulate --debug` JSONL.
For example:

```sh
svm-replay --demo --pretty
svm-replay --tx SIGNATURE --fields status,logs,computeUnits
svm-replay --tx SIGNATURE --collect '{"calls":true}' --pretty --out ./results
```

Default stdout contains the receipt plus `result.original`, optional
`result.replacement` or `result.requestedOverrides`, logs, CU and account effects.
Combined payload/account changes appear in `requestedOverrides`; the separate
original-control result stays unmodified. `accountOverride` is a legacy result field.
`--collect '{"calls":true}'` adds CPI/runtime-call exports under `trace.exports[].data`,
including completeness metadata; it does not request unlimited instruction tracing.
`--out DIRECTORY` saves that JSON to `timestamp-signature.json` and returns its path.
It saves typed failures too, including missing credentials or unsupported runtime/state.
Failures before execution have no engine receiptPath. Invalid command syntax and an
unwritable destination still report directly to the terminal. The SDK does not
return paths to its temporary output files.
Formatting does not change stored evidence receipts or their hashes. Pretty JSON
can be much larger, especially with register/memory arrays; prefer compact JSON
for large automated exports. Local `result/` and `results/` exports are ignored by
Git and source-layout checks.
Terminal JSON has a 512 MiB output bound; use `--out` for larger captures. Exceeding
a bound is an explicit error, never silent truncation of the result JSON.
`--human` changes presentation only. `--fields` selects status, logs, computeUnits,
accountTransitions or trace; verification/errors remain visible.
`--collect` controls additional capture; `--fields` only controls returned data.
Omit both for standard results, or use `--collect all` for every supported capture
category, complete or an explicit error. Setting collection limits opts into labelled
partial output. See [all collection settings](tracing.md#collection-settings).
Selecting `trace` requires an enabled `--collect` category. Excluded trace
payloads are not loaded; artifact metadata and verification remain visible.
`--human` shows up to 20 logs and 40 calls per export, completeness and file paths.
Interactive signature debugging is not part of this release. Use `--collect '{"calls":true}'` to
inspect saved execution data; the low-level debugger backend remains experimental.

Optional controls (defaults require none of these):

```sh
svm-replay --tx SIGNATURE --timeout 120 --max-requests 500
svm-replay --tx SIGNATURE --collect '{"calls":true,"maxEvents":10000}' --human
svm-replay --tx SIGNATURE --source captured.json --source-sha256 SHA256 --bank-input epochStakeEvidence
svm-replay --tx SIGNATURE --runtime-registry reviewed-registry.json --runtime-registry-sha256 SHA256
svm-replay doctor
```

`--timeout` is 1–3600 seconds across discovery and execution (default 900).
Large historical reconstructions can use `--timeout 1800`; this does not raise
provider request/byte budgets or the separate capture deadline. Cancellation is
cooperative: an in-flight HTTP request may take up to 30 seconds to return;
synchronous validation also stops at checkpoints. `--max-download-bytes` and
`--max-output-bytes` bound acquisition and worker output. Defaults: 5,000 RPC
requests (including account reads), 512 MiB downloaded, 256 MiB worker output.
Use `--max-requests 10000` for a larger acquisition budget; there is no separate
hidden account-read cap. Larger budgets can incur additional provider charges.
Budgets are not a provider price quote.

Captured sources retain the [provider evidence contract](providers.md). Bank inputs
are evidence names in that source, not raw account overrides or file paths:
`initializedStakeEvidence`, `epochStakeEvidence`, `initializationEvidence`,
`programMigrationEvidence`. Supplied data must match the exact network/slot/Bank
context. A custom registry must be independently reviewed and pinned; it cannot
create worker support or prove compatibility by itself. Discovery still uses
Alchemy. No missing inputs are inferred from current state.

`--replace` accepts `{"transactionBase64":"BASE64_SERIALIZED_TRANSACTION"}`.
`--overrides` accepts e.g. `[{"pubkey":"ACCOUNT_ADDRESS","lamports":"1000000000"}]`;
`dataBase64` patches account data, including supported program-code overrides.
Use inline JSON or prefix a filename with `@`: `@./payload.json`, `@../payload.json`,
or `@/absolute/path/payload.json`. This is a JSON file, not a file of raw base64.
Original-control inputs are never overwritten by these changes.

Acquisition defaults to 5,000 RPC requests including account reads, 512 MiB received,
32 MiB per response, and 900 seconds. This is a safety cap, not a price quote;
your Alchemy plan determines cost. Actual calls/bytes are in `sourceDiagnostics`.
See [measured cold examples](providers.md#measured-cold-examples), including the
explicit larger budgets needed by the complex Jupiter example.
Cache is on by default; `--no-cache` disables reusable input/result caching.
Finalized signature/block discovery is cached too, bound to the provider and
network. A fully warm supported replay can make zero RPC calls; new dependencies
still require fetching. Corrupt cached evidence rejects instead of silently
refetching. The Alchemy key is still required when selecting the live adapter.
Use `--data-dir PATH` to choose storage; missing inputs remain explicit outcomes.

## Advanced prepared/historical requests

```sh
./target/release/svm-replay simulate \
  --request historical-request.json \
  --catalog catalog.json --catalog-sha256 "$CATALOG_SHA256" \
  --source captured/manifest.json --source-sha256 "$SOURCE_SHA256" \
  --data-dir /absolute/path/to/replay-data --cache all
```

Historical requests reconstruct state. Prepared requests supply a reviewed
boundary; adding a source enables guarded hydration, not a claim of reconstruction.
Missing exact inputs produce typed outcomes.

--cache prepared reuses inputs and prepared boundaries; --cache all additionally
reuses verified results. A hit is labelled reuse, not a new execution. Identities
bind context, source, runtime, overrides and the actual coordinating binary.
Cached checkpoints contain account images and can exceed the small result size.
Set an explicit sufficient maxOutputBytes for that request, or select cache off;
the engine will not silently raise a request's byte bound.

--alchemy source-config.json selects the bounded live adapter. Only
API_ALCHEMY supplies its credential. Coverage and budgets are explicit.
Captured/live sources can combine; disagreement is an error.
[Provider fields, credentials and budgets](providers.md).

## Advanced prepared-request trace and debug

For the normal `--tx` workflow, use `--collect`; the following flags belong only
to `simulate` and use its separate nested capture/bounds contract.

`simulate --trace trace-options.json` requires separately pinned observation workers and
fresh reference/off/capture parity. Completeness is separate from correctness.
[Trace options](tracing.md).

Experimental backend only: for a settled prepared request, --debug --trace debug-options.json accepts JSONL
commands and emits live events. EOF cancels. It is not a graphical IDE.
[Debugger commands](tracing.md).

## Rust

Use svm_replay_engine::{simulate_historical, simulate_prepared,
simulate_prepared_with_sources, debug_prepared, Config}. Resolve Config.data_dir
to an absolute path. CLI and embedded callers share execution, cleanup and receipts.
TypeScript callers use the locally installable [SDK](../src/bindings/typescript/README.md).

## Request construction

Choose the input mode before running:

| Input | Meaning / source |
|---|---|
| Prepared request | Exact supplied account/Bank boundary plus original evidence. The installed demo is one example. It does not fetch the entire boundary. |
| Historical request | Slot, transaction index, original block pin and reviewed runtime binding. The engine reconstructs state using configured sources. |
| Runtime binding | Explicit slot, active features and pinned executor identity, reviewed for that boundary. A worker family name alone is insufficient. |
| Captured source | Hash-pinned local manifest and records, exported by an authorized data owner. Not just a getBlock JSON response. |

The advanced builder accepts a binding reviewed for **that slot**; do not edit a
demo's slot or copy its binding. The signature workflow above builds this request
automatically within the bundled reviewed interval. Outside that interval it
rejects rather than selecting the newest or nearest runtime.
The [protocol types](../src/protocol/_0_historical/mod.rs) define the required fields;
the CLI validates them before execution. No example invents historical inputs.

```sh
svm-replay request --candidate candidate.json --runtime-binding runtime.json \
  --family v3-0 --genesis-hash "$GENESIS_HASH" --request-id my-replay \
  --output historical-request.json
svm-replay validate --request historical-request.json
```

If you already have a full archived `getBlock` JSON response, replace
`--candidate candidate.json` with `--block block.json --signature "$SIGNATURE"`.
The builder finds the unique transaction index, hashes the original block bytes,
and checks the complete block before writing. It still requires a reviewed
runtime binding for that slot. To fetch the block instead, use
`--alchemy source-config.json --source-output acquisition.json --signature "$SIGNATURE"`
in place of `--block`/`--candidate`. Set `API_ALCHEMY`; the configured network,
coverage and budgets are enforced. Acquisition evidence is saved separately,
including raw response bytes and hashes. Neither mode discovers runtime eras or
claims account reconstruction: pass the request and source options to `simulate`
to reconstruct accounts and execute. A block fetch pin is provider evidence, not
an independent consensus proof.

The candidate contains the exact slot/transaction boundary, block identity and
reviewed runtime binding. Construction defaults to STRICT and refuses to overwrite
an existing file. Validation does not fetch data or claim the request can execute.
For an existing archived-meter warning policy, supply a separately reviewed request;
the builder never chooses a lenient policy automatically.

CLI exit 0 means COMPLETED; typed engine failures exit 1. Argument syntax errors
are reported by the CLI parser. COMPLETED means the simulation was valid, not that
the simulated transaction succeeded. Inspect the original/replacement status in
the result as well. The SDK returns guarded engine outcomes and throws transport,
integrity, deadline and cancellation errors with explicit codes.

Keep payloads, account data and saved debugging sessions private.

### Local bug-report draft

Add `--bug-report report.md` to a CLI command to create a local Markdown draft
if it returns a typed `INTERNAL` error. The draft contains only the CLI version,
OS, architecture, error code and a GitHub issue link. Review it and add sanitized
reproduction steps before submitting. Nothing is uploaded or opened automatically;
existing files are never overwritten. Report-writing failures go to stderr without
changing the original outcome. Expected failures, unsupported inputs and transaction
failures do not create drafts. Panics and process crashes are not captured by this
small helper. Report security vulnerabilities privately via SECURITY.md.

## Read the result

In a terminal, `svm-replay` opens an arrow-key menu (Enter selects; Escape cancels).
The normal replay entry collects a transaction signature, provider settings,
optional replacement/overrides, and collection settings (`all`, JSON or `@file`;
Enter keeps standard results). The advanced entry instead
collects an existing prepared request and pinned runtime manifest. It previews the
equivalent command before execution. In the advanced wizard, `/back` restarts
input collection; `/cancel` exits either wizard. The installation entry runs a
pinned health check. Signature replay has an optional settings screen for
fields, cache, data directory and budgets;
manual recovery source/registry controls remain explicit CLI flags.
`svm-replay demo` selects the bundled example, displays its transaction and slot,
then shows timed progress and a readable result. This uses cached historical inputs,
not live acquisition. `--example default` skips selection. `--json` disables menus
and preserves machine output; redirected output remains JSON by default.
Demo progress uses stderr when that stream is a terminal, even with `--json` or
stdout redirected to a file. Captured stderr and environments with `CI` set stay
quiet in JSON mode.
`simulate --human` opts into a readable final summary; it cannot be combined with
the JSONL `--debug` session. Human output uses yellow numbers, cyan hashes, blue
paths, magenta booleans, green success and red failure markers. `NO_COLOR` disables
colors; redirected output and JSON remain uncolored. The advanced menu loads an
existing prepared replay JSON; use the signature option for normal replay. Ctrl+C during replay
uses existing cooperative cancellation. Progress describes actual phases, not
estimated percentages. Detailed account changes remain in the result artifact.

The advanced `simulate` CLI prints a receipt with an `outcome`, a `receiptPath` and, on completion,
an `output` reference containing `file` and `sha256`. Resolve that file relative
to the receipt directory. The SDK's `simulate()` returns a verified `{path, sha256, bytes}` handle;
use `readArtifact` with an explicit byte bound to load it. Preserve wide integers
with a lossless JSON parser. This is a file reference, not a parsed transaction.
The normal `--tx` CLI additionally includes materialized results and selected
capture data; SDK `replay()` exposes that JSON as `response` alongside its receipt
and verified artifact handle. Early errors can lack `receiptPath`.

| Outcome | What to do |
|---|---|
| COMPLETED | Inspect transaction status, effects and verification warnings; the transaction itself may have failed. |
| NEEDS_INPUT | Supply the specifically requested exact historical evidence and retry. |
| UNSUPPORTED | The runtime/input path is not supported; changing to current state is not a fix. |
| MISMATCH | Preserve the receipt; original/control or execution expectations differ. Do not overwrite the baseline. |
| TIMEOUT / CANCELLED | Review the deadline/cancellation and retained artifacts before retrying. |
| ERROR | Read `error.code` and `error.message`; fix configuration, integrity or I/O errors. |

### Incomplete replays

Once a job starts, non-completed receipts also contain `incomplete`:

```json
{
  "category": "HISTORICAL_ACCOUNT_MISSING",
  "input": {"pubkey": "ACCOUNT_ADDRESS", "slot": 123},
  "complete": false,
  "estimated": false,
  "requestedReplayVerified": false,
  "originalControlVerified": false,
  "available": {}
}
```

This is a shortened example. `available` lists retained source observations,
preparation, failed attempts, traces or unverified control output **when present**.
It does not promise a trace or result was produced. Artifact paths are relative
to the receipt directory; check each SHA-256 before reading. The SDK exports
`readArtifact` for this. Saved JSON keeps the same evidence and `--human` shows
the category. No new flag is needed.

| Category | Meaning |
|---|---|
| `HISTORICAL_ACCOUNT_MISSING` | Required historical account fields are unavailable. |
| `SLOT_HASHES_MISSING` / `SYSVAR_MISSING` | Exact historical sysvar data is missing. |
| `BANK_CONTEXT_MISSING` | Required Bank, epoch/stake or reward context is unsupported/missing. |
| `HISTORICAL_DATA_UNAVAILABLE` | Another exact input is missing; inspect the original error. |
| `RUNTIME_UNSUPPORTED` / `CAPABILITY_UNSUPPORTED` | Runtime/platform, transaction format or requested capability is not supported; not proof of missing provider data. |
| `DEPENDENCY_UNSUPPORTED` | An ALT, program or nonce lifecycle path is unsupported. |
| `RESOURCE_LIMIT` / `PROVIDER_ERROR` | A configured bound or provider request failed; not proof that history is absent. |
| `VERIFICATION_MISMATCH` / `TIMEOUT` / `CANCELLED` | Verification failed or execution stopped. |
| `UNCLASSIFIED` | No reviewed category; the original code/details are preserved. |

Only the **first blocker** is reported; resolving it may reveal another.
Partial evidence is not an estimate or a verified requested result. An original
control can pass while a replacement fails; its verification stays separate.
Early configuration/admission errors can have no engine receipt or incomplete
envelope. Outcomes and nonzero failure exit codes are unchanged.

For example, a successful *demo* returns `outcome: "COMPLETED"`, `matched: true`,
`execution: "fresh-offline-supplied-state"` and a receipt path. Its `matched` flag
means the freshly computed output matched the pinned baseline, not that all
historical transactions are supported. CU warnings and trace truncation remain explicit.
