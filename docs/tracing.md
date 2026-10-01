# Trace exports

With `API_ALCHEMY` configured:

```sh
svm-replay --tx SIGNATURE --collect '{"calls":true}' --out ./results
svm-replay --tx SIGNATURE --collect '{"calls":true}' --human
```

This saves CPI/runtime-call data with completeness metadata. It is not
a complete instruction-by-instruction trace. Advanced capture settings are below.
There is no end-user interactive debugger in this release. The retained Rust and
hidden `simulate --debug` JSONL backend is experimental; session and idle timeouts
apply. It is not a supported human inspection workflow.

## Collection settings

Use `--collect all`, inline JSON, or `--collect @capture.json`.
Terminal JSON is indented automatically; `--out` files remain compact unless you
add `--pretty`. Whitespace can greatly expand
memory/register arrays: the tested full DeFi export grew from about 128 MB to
667 MB. Compact JSON is better for large files and automated consumers.
The old signature flags `--trace` and `--trace-options` are removed.
Unknown fields and invalid limits reject before provider access.

Without explicit collection limits, requested data must be complete: `--collect all`
does not stop successfully at 10,000 memory rows. If a safety ceiling prevents
completion, the command fails with `COLLECTION_INCOMPLETE` (or the relevant
timeout/output error); partial artifacts remain labelled for diagnosis.
Setting a collection limit explicitly opts into partial output, labelled `TRUNCATED`.
`all` means every supported category, not native builtin internals or host memory.

| JSON field | Default | Meaning |
|---|---|---|
| calls | false | CPI/runtime-call details |
| instructions | false | VM positions and register rows (captured together) |
| registers | false | Same VM positions/register capture |
| memory | false | Actual SBPF scalar loads/stores; implies instruction/register capture |
| programIds | [] | Select program payloads; empty selects all |
| instructionIndices | [] | Select zero-based outer instructions; empty selects all |
| executionMode | null / omitted | Platform default: jit on non-Windows x86_64; interpreter on ARM/Windows. Explicit jit/interpreter also accepted |
| maxBytes | 268435456 | Safety ceiling; set a smaller value to allow bounded partial capture |
| maxEvents | 1000000 | Safety ceiling; explicit values allow partial capture |
| timeoutMs | 180000 | Capture deadline; maximum 900000 |
| registerRows | 2000000 with SBPF | Producer safety ceiling; explicit values allow partial capture |
| memoryRows | 2000000 with memory | Producer safety ceiling; explicit values allow partial capture |
| maxInvocations | 1024 | Retained invocation bound; maximum 1024 |

Large captures on slower machines may need a longer deadline. Increase both the
capture and overall request budgets explicitly, for example:

```sh
svm-replay --tx SIGNATURE --timeout 900 --collect '{"calls":true,"memory":true,"timeoutMs":600000}'
```

Defaults are unchanged. Cancellation and the overall request deadline still apply;
check the reported completeness when supplying explicit collection limits.

Instruction/register/memory capture retains call context. Omitted booleans are
false, but do not suppress dependencies. Rows bound retained observations, not an
unlimited execution history. Native builtins do not expose SBPF internals.
Filters may retain omitted-call markers to preserve nesting. No enabled category
(`{}`) means standard replay. Non-null row limits without their category are errors;
omit them or use null when disabled. Instruction filters without any enabled
category reject. Byte/event/time/invocation limits must be positive; row bounds
may be zero to retain no rows of that kind.
The overall CLI timeout/output budgets also apply. Worker capabilities are checked;
unsupported requests fail without silently changing mode.

Example `capture.json`, with every category enabled and explicit limits:

```json
{
  "calls": true,
  "instructions": true,
  "registers": true,
  "memory": true,
  "programIds": [],
  "instructionIndices": [],
  "maxBytes": 67108864,
  "maxEvents": 100000,
  "timeoutMs": 180000,
  "registerRows": 100000,
  "memoryRows": 10000,
  "maxInvocations": 1024
}
```

Standard output contains status/logs/CU/account transitions and verification.
Collection adds `trace.exports[].data`: call journal events, and requested bounded
SBPF observations. Inspect completeness: `TRUNCATED` means missing observations,
not a complete trace. Human output is only a bounded preview.
See the [README examples](../README.md#choose-additional-data).

Memory rows identify invocation, program counter, address, access width, load/store,
success/fault and observed bytes. Successful stores include before/after bytes.
Some runtimes also include snapshots for loads; these must both equal the loaded
value. Inconsistent read snapshots are rejected, not treated as writes.
Bytes are padded to eight; only the first `width` bytes are meaningful.
Integers outside JavaScript's safe range are exported as exact decimal strings,
including wide register values. They are converted before parsing in JavaScript,
never after rounding. Ordinary counters and byte arrays remain numbers.
Register and memory categories are interleaved to prevent one starving the other;
each preserves its own order, but this is not a merged chronological timeline.
Large instruction traces can produce tens or hundreds of MB: prefer `--out ./results`.
`maxBytes` bounds the trace journal/export; the separate `--max-output-bytes`
ceiling covers the raw worker response, including its execution-result envelope.
Worker catalogs must explicitly advertise memory support; older bundles may reject it.

## Advanced prepared-request backend

This retains its separate engine capture contract for existing prepared integrations;
do not pass that nested object to `--collect`.
Use `simulate --trace trace-options.json` with separately pinned observation
workers. Reference, capture-off and capture-enabled executions must agree before
exports are accepted. The original historical control precedes modified execution.
The advanced `TraceOptions` contract can set `require_complete: true`; its default
is false because its capture/bounds object already supplies explicit limits.

Trace policies bound bytes, events, registers, memory and invocation retention.
Inspect completeness: TRUNCATED is not a full instruction trace. Native builtin
internals and host-syscall memory copies are outside SBPF memory coverage.
Results identify execution mode, worker, evidence and artifact hashes.

Specify `executionMode` explicitly: `jit` for non-Windows x86_64 capture,
`interpreter` for native ARM/Windows capture, or `interpreter-debug` for a live
debug session. The pinned worker must declare the requested capability and its
output must confirm the mode; there is no silent JIT-to-interpreter fallback.
Platform qualification is listed in [validation](validation.md).

For an already settled prepared boundary, simulate --debug --trace OPTIONS
accepts DebugCommand JSONL on stdin and emits events followed by the final
receipt. Read the event's phase/target before issuing a command. Requests carry
a unique requestId, phase, optional target and tagged action. Keep stdin open;
EOF cancels. Interactive commands are not automatically replayed to hydrate
missing historical inputs. Prepare first, then start a fresh debug session.

Windows JSONL debugging requires piped stdin. Closing it cancels cooperatively.
Node's Windows `SIGTERM` forcibly terminates the process: worker Job Objects
provide cleanup, but a final cancellation receipt cannot be promised.

Stepping, breakpoints, registers and bounded memory inspection are available
through Rust and the CLI. Source navigation requires exact usable symbols.
There is no graphical IDE or true reverse execution. The TypeScript binding
currently exposes simulation/trace artifacts, not an interactive debugger client.

Versioned Rust types define the machine-readable contracts. See
[the protocol crate](../src/protocol/) and [debug commands](../src/engine/_shared/_3_debug/).
See [limitations](limitations.md) before interpreting coverage.

## Minimal capture configuration

Save as `trace-options.json` (call-level interpreter capture, not SBPF memory):

```json
{
  "capture": {
    "schema": "svm-capture-request/v2",
    "executionMode": "interpreter",
    "level": "calls",
    "sbpfObservations": "none",
    "filter": {"programIds": [], "instructionIndices": []},
    "limits": {"maxBytes": 16777216, "maxEvents": 10000, "timeoutMs": 60000}
  },
  "bounds": {"registerRows": 0, "memoryRows": null, "maxInvocations": 64}
}
```

The request's output/time budget must also accommodate the capture. Select `jit`
instead on qualified non-Windows x86_64 workers. A worker lacking the selected
capability rejects; do not infer a fallback from its OS alone.

## Live command example

Use `executionMode: "interpreter-debug"` with `simulate --debug --trace ...`.
A frontend keeps stdin open and reads events. When a paused invocation arrives,
copy its current `phase` and `target` into the command:

```js
child.stdin.write(JSON.stringify({
  requestId: 1, phase: currentPhase, target: event.target,
  action: {kind: "metadata"}
}) + "\n");
```

Here `currentPhase` is tracked from `phase-start` events. After the reply, send
a new request ID with `action: {kind: "step"}` or
`{kind: "continue"}`. Track `phase-start` and pause events; never reuse a stale
target after resuming. This is a protocol fragment, not a standalone debugger
client. Closing stdin cancels; a pipe containing one command then EOF will not
keep the session alive.
