# Why SVM Replay?

Standard Solana RPC returns old transactions and blocks, but not old account data. Running an
old transaction against today's accounts does not reproduce what happened. Replay needs the state
just before the transaction, the effects of earlier transactions in its block, and the runtime
rules of that time. SVM Replay rebuilds that state and checks the original before testing
changes. Alchemy's archive excludes vote accounts and per-slot sysvars;
[provider coverage](providers.md).
