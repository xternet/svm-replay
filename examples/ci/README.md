# CI template

workflow.yml runs the repository's structure, Rust, SDK and example checks
without historical-provider credentials.

This is a **maintainer CI template for this repository**, not a plug-in regression
workflow for your application. It deliberately matches the active development CI.

The active workflow is [.github/workflows/ci.yml](../../.github/workflows/ci.yml).
For another project, adapt the template's paths to that project's layout.

For an application, choose [ordinary Anchor](anchor-normal.yml) or
[offline historical Anchor](anchor-historical-offline.yml). Both require an
isolated Linux runner with reviewed tools, Node/npm, the SBF toolchain and a
committed npm lockfile. The ordinary recipe uses no SVM Replay backend. Configure
funding explicitly if the application uses a non-default wallet.

The offline historical recipe uses the generated template's `after: "413023260"`,
Alchemy adapter configuration and `.surfpool/history` cache. It installs a fresh
recorded cache, then disables provider networking during tests. Its dummy key
is only for configuration parsing; missing inputs fail rather than fetch mainnet.
The fixture URL and hash must refer to a separately reviewed, redistribution-cleared
artifact. No public URL is supplied yet. Never upload the mutated test cache.

For an Anchor application, [historical.yml](historical.yml) runs `anchor test`
with test-owned Surfpool forks. It requires an isolated self-hosted runner with
reviewed matching integration builds, their binary hashes in repository variables,
the native bundle and a provider secret. It does not download stock tools as a
substitute. Tests choose the historical boundary in code. Provider-side quotas
limit acquisition spend; a CI timeout alone is not a spending cap.

Historical checks are separate and opt-in: provide independently pinned workers,
requests and captured inputs, then invoke the historical example. Assert
transaction status/effects in addition to COMPLETED. Never publish secrets,
private payloads or account data as CI artifacts without review. For an application
regression, use the historical recipe and assert the expected transaction effects,
or run the installed offline demo to assert the demo's pinned full-output baseline.
