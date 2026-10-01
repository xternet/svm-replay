# SVM Replay SDK

Quick start with an already installed native bundle and `API_ALCHEMY` in your
environment or current-directory `.env`:

```ts
import { Replay } from "@xternet/svm-replay";
const replay = await Replay.installed("./svm-replay");
const result = await replay.replay({tx: "SIGNATURE", collect: {calls: true}});
console.log(result.response);
```

Use `collect: "all"` for all bounded capture categories, or select `calls`,
`instructions`, `registers`, and `memory` in the object. Optional filters and
limits use the same [collection settings](https://github.com/xternet/svm-replay/blob/main/docs/tracing.md#collection-settings) as the CLI.
Omit `collect` for standard results. Instruction/register observations are coupled;
memory includes both. The old signature `trace` option is rejected. Advanced
`simulate({request, trace: ...})` retains its nested capture/bounds configuration;
it is separate from signature `replay({tx, collect: ...})`.

`replay()` reconstructs automatically and returns typed outcomes; `simulate()`
retains the advanced request-based API. Signature options include replacement,
overrides, selected fields, cancellation, budgets and pinned recovery sources.
Inline replacement/override/capture objects use private temporary files, not
large command-line arguments. Returned result and trace data are checked against
their hashed artifacts. alchemyConfig applies to replay({tx}) as well as simulate;
do not combine it with per-call maxRequests/maxDownloadBytes.
Surfpool's companion accepts `{tx}`; `anchorSignature()` builds a replacement.

Node 22+ and Bun, calling the same pinned Rust CLI. No postinstall download or ecosystem peer
dependency. Supply a native bundle matching your OS and CPU. Linux requires GNU
libc >= 2.39; macOS uses system libraries; Windows needs the matching MSVC v14
runtime. See [installation](https://github.com/xternet/svm-replay/blob/main/docs/installation.md)
and [validation](https://github.com/xternet/svm-replay/blob/main/docs/validation.md).

The package also provides `svm-replay-install --help`: an explicit offline native
installer using a release manifest, its independently trusted SHA-256 and the
matching archive. Requires Node 22+ and `tar`; never runs as a postinstall hook.

```js
import {Replay} from '@xternet/svm-replay';

const replay = await Replay.open({
  bundlePath: '/path/to/install/bundle.json',
  bundleSha256: 'independently-reviewed-sha256',
  dataDir: '/path/to/writable-data',
  source: {path: '/path/to/captured/manifest.json', sha256: 'reviewed-source-sha256'},
  cache: 'all',
});
const {receipt, result} = await replay.simulate({request: {path: 'historical-request.json'}});
if (receipt.outcome !== 'COMPLETED') throw new Error(JSON.stringify(receipt.error));
console.log(result); // Verified file path/hash/size, not a huge parsed object.
```

Use historicalRequest() to build a request from reviewed candidate/runtime inputs.
Rust validates schema, runtime binding and actual execution. The builder cannot
discover unavailable Bank inputs or prove an arbitrary runtime profile correct.
Keep u64 balances as decimal strings; floats/unsafe numbers/bigints reject.

readArtifact(root, reference, maxBytes) explicitly reads and verifies an artifact.
verifyArtifact() streams verification without loading the entire file into JS.
Raw result/trace bytes are available; do not JSON.parse wide integers without an
appropriate lossless decoder. Completed simulation does not imply program success.

Pass signal: AbortSignal to cancel. The SDK waits for CLI closure; Rust owns worker
cleanup. Transport/bounds/cancellation failures throw ReplayError. Engine guarded
outcomes return their original typed receipt. Provider credentials never enter a
request: only API_ALCHEMY is forwarded when Alchemy is selected.
File-access failures retain OS error codes such as ENOENT; they are never converted
to a cache miss or an empty result.

Optional companions: import withHistoricalReplay, anchorSignature, anchorRequest, assertCompleted
from '@xternet/svm-replay/adapters'. They do not use a consumer's current-state runtime
as historical authority. See [examples](https://github.com/xternet/svm-replay/tree/main/examples)
and [integrations](https://github.com/xternet/svm-replay/blob/main/docs/integrations.md).
