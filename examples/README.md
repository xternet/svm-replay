# Examples

**First try [the offline demo](../docs/installation.md#cached-offline-demo).**
The recipes below accept a transaction signature or advanced prepared inputs.

Each directory is one consumer recipe. They share one JavaScript dependency
installation; they are not separate repositories or replay engines.

| Directory | Entry | Purpose |
| --- | --- | --- |
| [historical/](historical/README.md) | main.mjs | Replay a signature or reviewed historical request |
| [surfpool/](surfpool/README.md) | main.mjs | Use the historical companion alongside Surfpool |
| [anchor/](anchor/README.md) | main.mjs | Encode and simulate a replacement transaction |
| [rust/](rust/README.md) | main.rs | Embed the native engine |
| [ci/](ci/README.md) | workflow.yml | Run offline repository checks |
| config/ | *.example.json | Non-secret configuration templates |
| shared/ | config.mjs | Shared example configuration loader |

## Setup

From the repository root:

```sh
npm --prefix src/bindings/typescript ci --ignore-scripts
npm --prefix src/bindings/typescript run build
npm --prefix examples ci --ignore-scripts
```

Copy config/signature.example.json to config/replay.local.json. Set your transaction
signature, installed bundle path and its installation.json bundleSha256. Export
API_ALCHEMY in your shell; live reads may incur provider charges. No prepared
request is needed for this route. Missing historical inputs return typed outcomes.

For advanced offline inputs, use config/replay.example.json instead and supply
reviewed request, worker-bundle and captured-source paths/pins.
Local configuration is Git-ignored. Relative paths resolve against the
configuration file, not your shell's working directory. Omit dataDir to use the
OS application-data directory.

```sh
npm --prefix examples run historical -- /absolute/path/to/replay.local.json
npm --prefix examples test
```

These are recipes, not bundled historical datasets. Use the workers from your
verified native installation.
The demo is supplied in eligible native archives, not in this examples directory.

The signature recipe prints a receipt, materialized result and verified result-file
reference; prepared requests return the receipt and file reference. Check transaction
status and expected effects. `COMPLETED` alone is not a business-logic assertion. The demo instead
checks its entire output against the pinned baseline automatically.

## Live Alchemy instead of captured inputs

Set API_ALCHEMY in the calling process. In installation, replace source with
alchemyConfig pointing to your completed alchemy.local.json. Copy the provider
template and fill its genesis hash and integer firstSlot/lastSlot values; null
placeholders intentionally reject. Review budgets before enabling live reads.
Both sources can be explicitly configured; conflicting observations reject.

See [provider configuration](../docs/providers.md). The key never belongs in JSON.
Offline examples need no key, wallet, signing or transaction broadcast.
