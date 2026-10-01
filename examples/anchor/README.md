# Anchor replacement transaction

Prerequisite: complete the [shared setup](../README.md). Add this transfer object
to your local replay configuration, replacing all placeholders:

```json
{
  "transfer": {
    "payer": "HISTORICAL_PAYER_PUBLIC_KEY",
    "recipient": "HISTORICAL_RECIPIENT_PUBLIC_KEY",
    "blockhash": "HISTORICAL_BLOCKHASH",
    "lamports": "1"
  }
}
```

```sh
npm --prefix examples run anchor -- /absolute/path/to/replay.local.json
```

main.mjs encodes a replacement through Anchor using transfer.mjs and an explicit
System Program IDL. Accounts, payer and blockhash are supplied, not fetched from
current RPC. Set tx for automatic historical reconstruction, or requestPath for
an advanced prepared request. It performs no signing/broadcast and
does not need a wallet. Missing historical inputs remain typed engine outcomes.

The optional Anchor.toml runs main.mjs with ../config/replay.local.json when
launched from this directory. The tested Node example does not need the Anchor CLI.
