# Capabilities and interface gaps

Source audit of the release candidate. Implemented does not mean every historical
transaction, provider or platform path has been qualified. See [validation](validation.md).
The unfinished keyboard debugger is excluded; the engine and hidden JSONL bridge
remain experimental for future work.

## User-facing inventory

| Function | Entry point | Current behavior |
|---|---|---|
| Replay by signature | `--tx SIGNATURE` | Live lookup, reviewed runtime selection, historical reconstruction and original verification |
| Replace transaction | `--replace JSON` or `--replace @file` | Serialized `transactionBase64`; original verified before the variant |
| Override accounts/programs | `--overrides JSON` or `--overrides @file` | Lamports and raw account bytes; program/sysvar/ALT/nonce/stake/vote rules are checked, not arbitrary field editing |
| Combine changes | `--tx ... --replace ... --overrides ...` | Requested changes stay separate from original-control evidence |
| Export call traces | `--collect '{"calls":true}'` | Complete CPI/runtime-call export or an error; not every VM instruction |
| Detailed collection | `--collect all` or `--collect JSON/@file` | Calls, VM positions/registers and SBPF memory; complete by default, explicit limits allow partial output |
| Save JSON | `--out DIRECTORY` | Full selected result saved as timestamp-signature JSON; terminal returns the path |
| JSON formatting | Automatic / `--pretty` / `--json` | Terminal JSON is indented; pipes/files stay compact unless `--pretty`; `--json` forces compact output |
| Select result fields | `--fields ...` | Filters transaction fields and skips excluded trace payloads; keeps verification/artifact metadata |
| Readable summary | `--human` | Selected result, logs, first three changed accounts, bounded call tree, completeness and artifact paths |
| Offline example | `--demo` / `demo` | Executes bundled cached inputs; no API key required |
| Guided setup | no arguments in a terminal | Demo, signature replay, advanced prepared request, help |
| Credentials | `API_ALCHEMY`, current-directory `.env`, `--alchemy-key-file` | Key file > exported environment > local `.env`; no parent-directory search |
| Cache and storage | `--no-cache`, `--data-dir`; advanced `--cache` | Reusable finalized discovery, inputs/results and private receipts; fully warm calls can avoid RPC |
| Advanced reconstruction | `request`, `simulate`, `validate` | Explicit historical/prepared requests, Bank inputs, captured/live sources and trusted pins |
| Installation diagnostics | `doctor`, `capabilities` | Automatic installed-pin verification; explicit catalog/bundle still accepted |
| Bundle distribution | `bundle pack`, `bundle install`, npm installer | Local pinned packages; opt-in npm command activation, no automatic latest-worker download |
| Issue draft | `--bug-report FILE` | Safe local draft for typed INTERNAL failures only; no upload or automatic PR |
| Rust embedding | engine public API | Prepared/historical simulation, source hydration, cancellation and receipts |
| Incomplete evidence | Automatic on non-completed jobs | Typed category, missing-input details and retained artifact references; no estimated or verified requested result |
| TypeScript embedding | `Replay.installed`, `replay({tx})`, `Replay.open`, `simulate` | Signature and detailed request APIs through the same native CLI |
| Surfpool/Anchor/CI | SDK companions and examples | Wrapper, transaction construction and assertions; not upstream integration |
| Experimental debugger | Rust / hidden `simulate --debug` | Machine-readable prepared-session backend; not release UI |

Historical reconstruction includes dependency discovery, predecessor replay and
guarded runtime/Bank/sysvar/program/ALT/nonce/stake/reward handling. Missing exact
inputs reject explicitly. This is not a promise of universal automatic recovery.
Receipts record identities, verification, source evidence and typed outcomes.

## Interface audit follow-up

| Area | Implemented | Deliberate limits |
|---|---|---|
| Trace capture | Native-mode default; complete-or-error unless limits are explicit | Safety ceilings and worker capability checks remain; see tracing guide |
| Trace display | Logs, nested calls, completeness and export paths in `--human` | Bounded preview, not a live debugger or source-level IDE |
| Field selection | JSON and human honor selection; excluded trace data is not loaded | Evidence metadata stays visible |
| SDK/companions | Signature API, installed-manifest convenience, Surfpool `{tx}`, Anchor replacement | Companions, not upstream acceptance; SDK still requires a native installation |
| SDK integrity | Private files for large inputs; returned result/trace values checked against hashed artifacts | Native input/output bounds still apply |
| Provider configuration | `--alchemy-config`; SDK instance/per-call alchemyConfig | Cannot combine provider JSON with request/download CLI budget overrides |
| Budgets/cancellation | Early signal handling, provider checkpoints, total timeout, request/download/output controls | In-flight HTTP bounded to 30s; synchronous work stops at checkpoints |
| Installation | Automatic installed doctor; opt-in npm launcher activation | Requires npm global bin on PATH or explicit native path; no shell edits |
| Exact recovery inputs | Pinned captured source, named Bank evidence, pinned custom reviewed registry | Alchemy discovery remains; missing evidence/runtime support is not invented |
| Guided replay | Optional fields/cache/directory/budget/capture settings; issue-draft hook | Specialist recovery flags remain CLI-only |

Other deliberate limits: bundled runtime coverage is slots 353951234–451646918 using
Alchemy; no transaction broadcast, wallet management, HTTP service, graphical UI,
Python package, token-unit override helper, instruction builder or batch scheduler.
Other languages can call the JSON CLI. New profiles/data require explicit review.

These interface changes require release-candidate review and renewed platform
qualification; older platform results are not proof for this changed CLI.
