# Historical replay

Prerequisite: complete the [shared setup](../README.md).

```sh
npm --prefix examples run historical -- /absolute/path/to/replay.local.json
```

main.mjs calls replay({tx}) for signature configurations, or simulate for a
reviewed requestPath. It prints the typed receipt and verified result-file
reference (plus materialized output for signatures). Inspect the result's
transaction status/effects: COMPLETED does not mean the transaction succeeded.

For a deterministic CI regression, assert the reviewed full-output hash after
simulation (use a baseline approved for the same runtime/platform and request):

```js
import assert from "node:assert/strict";
const result = assertCompleted(await replay.simulate(options));
assert.equal(result.result.sha256, reviewedExpectedOutputSha256);
```

Never replace the expected hash automatically after a failure. For a first trial
without your own inputs, use [the offline demo](../../docs/installation.md#cached-offline-demo),
which already performs this baseline comparison.

## Benchmark targets

[cases.json](cases.json) lists the 47 reviewed transaction boundaries, runtime
families, metadata policies and retained request/result SHA-256 pins. These are
public identifiers and qualification references, not executable requests.

To reproduce a target, obtain a reviewed request/runtime binding and the required
historical inputs through your own authorized provider access, or use an approved
captured/prepared request. The index alone cannot reconstruct Bank inputs. See
[provider setup](../../docs/providers.md) and [request formats](../../docs/usage.md).

Provider-supplied account snapshots and full captured responses are not included
in this repository. Their redistribution permissions still need confirmation.
Offline replay needs no API key once all reviewed inputs are present; live
acquisition may incur provider costs. The retained request pin applies to the
original serialized fixture, not arbitrary re-serialization or newly fetched data.
