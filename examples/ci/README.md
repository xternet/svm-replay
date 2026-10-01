# CI template

workflow.yml runs the repository's structure, Rust, SDK and example checks
without historical-provider credentials.

This is a **maintainer CI template for this repository**, not a plug-in regression
workflow for your application. It deliberately matches the active development CI.

The active workflow is [.github/workflows/ci.yml](../../.github/workflows/ci.yml).
For another project, adapt the template's paths to that project's layout.

Historical checks are separate and opt-in: provide independently pinned workers,
requests and captured inputs, then invoke the historical example. Assert
transaction status/effects in addition to COMPLETED. Never publish secrets,
private payloads or account data as CI artifacts without review. For an application
regression, use the historical recipe and assert the expected transaction effects,
or run the installed offline demo to assert the demo's pinned full-output baseline.
