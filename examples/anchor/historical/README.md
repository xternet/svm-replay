# Legacy datasource fixture

This directory preserves the earlier account-datasource experiment and its
recorded tests. It is **not the full historical Anchor integration** and is not
the recommended starting template.

It ran Surfpool's current VM against historical account images. That cannot
establish historical runtime equivalence. Its old hooks, `--match-datasource`
settings and expectation format are retained only for those legacy test drivers.

For ordinary Anchor tests, use the integration build:

```sh
anchor init my-project --test-template mocha --historical
cd my-project
anchor build --arch v0
anchor test --skip-deploy
```

Configure the provider in `Anchor.toml`; select the boundary in each test's
generated helper. Surfpool owns RPC/WS and the chain, while SVM Replay executes
the historical VM. See [the current integration guide](../../../docs/integrations.md#anchor).
