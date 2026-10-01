# Surfpool companion

Prerequisite: complete the [shared setup](../README.md) and install Surfpool's
required native executable for your platform.

```sh
npm --prefix examples run surfpool -- /absolute/path/to/replay.local.json
```

main.mjs starts an owned offline Surfpool instance, wraps it with
withHistoricalReplay, calls simulateTransactionAt, then stops that instance.
Set tx in the shared configuration for automatic reconstruction; requestPath
remains available for advanced prepared inputs.

Historical execution happens in SVM Replay, not Surfpool's current Bank.
This is our companion integration, not an upstream feature or endorsement.
