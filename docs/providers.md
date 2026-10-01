# Historical data providers

## Offline

Prepared boundaries or captured-source manifests can run without an API key.
They still need reviewed worker/runtime inputs and independent SHA-256 pins.
Missing inputs are not filled from current-state RPC.

## Alchemy

Provide a key with Solana Account Archive access using the standard environment
variable API_ALCHEMY. Set it in your shell/session or CI secret store.
The signature CLI (`--tx` and its interactive menu) also reads API_ALCHEMY from
`.env` in the current working directory. Priority: explicit `--alchemy-key-file`,
then existing environment variable, then `.env`. Only API_ALCHEMY is imported;
shell commands and variable expansion are not executed. The low-level prepared
request API uses an explicitly supplied environment; SDK replay({tx}) delegates
to the signature CLI, including its .env loading.
Never commit a real key.

If `getTransaction` returns null, lookup can use a finalized historical
`getSignatureStatuses` result and fetch the transaction from that slot's block.
The exact signature and serialized transaction are checked; raw provider
evidence is retained. This may require extra requests and does not recover
missing account state. Null status, inconsistent evidence and provider errors
remain explicit failures—there is no current-state fallback.

```sh
export API_ALCHEMY="YOUR_KEY"
svm-replay simulate --request historical-request.json \
  --bundle /path/to/bundle.json --bundle-sha256 VERIFIED_BUNDLE_SHA256 \
  --alchemy /path/to/alchemy.local.json
```

Complete [the provider template](../examples/config/alchemy.example.json):
id/version identify the provider configuration; genesisHash must identify the
verified network; firstSlot/lastSlot bound the requested historical coverage.
Replace nulls with integers. maxRequests, maxAccountReads, maxDownloadBytes and
maxResponseBytes explicitly bound resource use. These are not dollar budgets.
durableBudgetId is optional and shares a persistent, non-resetting reservation
ledger across calls. Failed attempts may retain their reservation.

For the TypeScript binding, set alchemyConfig when opening Replay, or for an
individual replay or simulate call. The signature CLI accepts the same file via
--alchemy-config; it cannot be combined with --max-requests or --max-download-bytes.
Surfpool and Anchor companions use the same configured
Replay instance. No additional credentials belong in their transaction objects.

Only the selected Alchemy credential is forwarded to the native CLI. Workers do
not receive provider credentials. The transport uses the fixed HTTPS Solana
mainnet Alchemy endpoint, with no redirects or automatic retries. Missing/invalid
keys fail explicitly; there is no legacy-variable or current-state fallback.
Selecting the live adapter requires the key even when some reads are cached.

No alternate credential variable is read. Keep credentials in your environment,
not JSON configuration, source files or receipts.

## Availability and cost

Alchemy access may incur provider charges. Confirm your account's archive access
and coverage using [Alchemy's documentation](https://www.alchemy.com/docs/solana/account-archive).
Archive accounts alone do not provide every historical Bank/epoch input.

The source API also accepts account queries with phase
`last-write-at-or-before-slot`. Alchemy uses its historical update cursor and
preserves the actual write slot as `sourceSlot`, with the raw RPC provenance.
This is a write observation, **not an end-of-slot image**: native Bank processing
may have additional same-slot writes. Its query/cache identity is separate from
`end-slot`; a missing archive entry is an explicit provider error, not proof of
account absence. Replay recovery must independently justify how to use it.
Coverage configuration is not proof that all data exists.

An archive null is not sufficient evidence of an empty account when the block's
transaction metadata shows a positive pre-balance. Such contradictory inputs
return `UNSUPPORTED_HISTORICAL_ACCOUNT` with the affected account and boundary;
excluded vote-account data can cause this. No account is invented from its balance.
An account introduced only by a replacement also needs evidence: a partial archive
null without an archived first zero pre-balance returns `UNSUPPORTED_HISTORICAL_ACCOUNT`
with reason `unproven-account-absence`. Explicit reviewed prepared inputs remain a
separate path; a null response alone does not prove that an account never existed.

For reviewed Agave 4.1.2 and 4.2.1 profiles, an active EpochRewards image at or
after the final distribution partition triggers a guarded recovery attempt.
Recovery requires a matching archived write at the exact final-partition block
height, historical Clock/Rent inputs, the same epoch, and reviewed runtime and
feature coverage through the target. It applies only the proven native terminal
transition, retaining raw inputs and a `terminalEpochRewards` proof in preparation
evidence. It never invents missing distribution increments. Missing or conflicting
proof inputs remain explicit errors or unsupported outcomes; no guessed flag or
balance is substituted. Other missing accounts can still prevent the replay.

For reviewed Agave 2.2.20, 2.3.9, 3.0.10, 3.1.11, 4.0.0, 4.1.2 and 4.2.1 profiles, exact inactive
EpochRewards evidence after the distribution interval can prove that parent
stake images need no pre-transaction reward update. Agave 2.2.20 and 2.3.9 additionally
requires the exempt rent marker. Active/final-distribution blocks, unbound inputs
and unreviewed eras do not use this recovery. Later execution can still encounter
other missing inputs; this proof is not a guarantee that the whole replay succeeds.

For the reviewed Agave 2.3.9, 3.0.10, 3.1.11, 4.0.0, 4.1.2 and 4.2.1 profiles,
a missing predecessor input can sometimes
be avoided: a present, empty System-owned account can be initialized from proven
historical credits before its first selected use. Owner/data changes, incomplete
inner-instruction records, debits and interleaved omitted credits reject this
recovery. The receipt retains `systemCreditPreloads`; prepared requests recheck
the proof. This does not recover missing vote-account bytes or SlotHashes.

Captured and live sources may be combined explicitly. Disagreement is an error.
Missing SlotHashes need not prevent lookup through a recently deactivated ALT:
the reviewed runtime families, including the private 2.2.20 candidate, can prove it remains in the history window
when fewer than 512 slots have elapsed. This proof preserves address warmup and
index checks; it does not supply hash bytes. Programs reading those bytes still
require exact historical SlotHashes.

Private runtime qualification can leave explicitly declared gaps between reviewed
profiles. A transaction in such a gap returns `UNSUPPORTED_RUNTIME`; there is no
nearest-version substitution. This is a runtime qualification limit, not evidence
that Alchemy lacks the transaction's historical data.

Receipts bind supplied observations; they are not independent consensus proofs.
New providers implement the Rust HistoricalSource interface, not per-tool logic.

### Measured cold examples

Local measurements on 27 September 2026, empty reusable cache / `--no-cache`:

| Example | RPC requests | Account reads | Download | Estimated API cost |
|---|---:|---:|---:|---:|
| SOL transfer | 16 | 11 | 8.98 MB | $0.00015 |
| DeFi replay | 131 | 126 | 31.33 MB | $0.00075 |
| Jupiter, 138 replayed predecessors | 3,183 | 3,040 | 217.70 MB | $0.01895 |

All three completed and passed original verification. Times were approximately
26 seconds, 63 seconds and 7 minutes 21 seconds on the test server; not latency guarantees.
The complex cold case used explicit `--timeout 900` and a provider configuration
allowing 5,000 requests / 4,800 account reads. The battle-test branch now defaults
to 900 seconds and 5,000 requests, with account reads sharing that request cap.
An earlier 300-second attempt timed out; failures can consume provider credits too.

These are estimates, not invoice measurements: attempted method counts multiplied
by [published method costs](https://www.alchemy.com/docs/reference/compute-unit-costs)
and the [published $0.525 per million CU PAYG rate](https://www.alchemy.com/docs/reference/pricing-plans).
The calculation assumes historical getAccountInfo uses the listed 10 CU rate;
confirm archive-specific billing and your plan with Alchemy. getGenesisHash is
10 CU; getBlock/getTransaction are 40 CU. Totals: 280 / 1,430 / 36,090 provider CU.
These are not Solana execution compute units. This small sample is not a worst-case bound.

Receipts expose attempted method counts in `sourceDiagnostics` under
`diagnostics.transport.methods`, including failures. `charged_bytes` is a resource
reservation counter, not money billed. Local trace capture itself calls no RPC;
historical acquisition may. A fully warm replay can make zero live calls.

### Credential and provider failures

| Condition | Result / action |
|---|---|
| Missing or malformed key | `SOURCE_CONFIGURATION`; configure API_ALCHEMY or a key file |
| HTTP 401 | `SOURCE_HTTP_ERROR`; check the key |
| HTTP 402 | `SOURCE_HTTP_ERROR`; check credits/billing |
| HTTP 403 | `SOURCE_HTTP_ERROR`; check key restrictions/archive access |
| HTTP 429 | `SOURCE_RATE_LIMIT`; check throughput/credits before retrying |

HTTP failures include safe status/hash metadata, not the provider's raw response
or your key. JSON-RPC failures remain explicit too. There are no automatic retries
or substitutions with current-state data.
