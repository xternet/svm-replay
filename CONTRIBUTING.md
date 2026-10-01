# Contributing

Read [architecture](docs/architecture.md), [tests](tests/README.md) and
[AGENTS.md](AGENTS.md). Version 0.1.0 is an unpublished release candidate.

- Keep entry points readable. Component logic/tests belong with their owner;
  src/engine/_shared/ is only for services shared by replay stages.
- Use mod.rs for module entries and numbered Rust components at every depth.
  Numbers express execution order or stable reading order, not invented sequencing.
  Preserve one workspace and the existing package names.
- Routing directories keep only mod.rs as production Rust source; group implementations in
  numbered responsibility-named child directories. Leaf components can hold related files.
- Owned tests use one unnumbered tests.rs beside their owner, or tests/ for multiple
  files/helpers. Do not wrap one Rust test file in a directory or use both forms.
  The recursive hierarchy guard rejects loose helpers at routing levels.
- Aim for ~50-line entries and ~200-line helpers. The layout check allows small
  overruns; extract a named responsibility rather than arbitrary numbered parts.
- High-level Rust APIs and wire schemas are integration contracts. Avoid coupling
  adapters to internal stages. Providers supply evidence, not correctness.
- Add failing tests before fixes. Preserve every Cargo test target when moving
  tests. Ordinary checks stay offline.
- Never substitute current state, invent Bank inputs, overwrite observed CU or
  loosen frozen historical policies. Preserve control/variant separation,
  typed failures, cancellation and worker cleanup.
- Current guides live in docs/; private historical logs stay outside the repository.
  Runtime data, binaries and bulk receipts stay outside Git.

Before editing behavior, run the affected tests to establish a baseline, then
add a failing regression test. rust-toolchain.toml pins Rust and rustfmt to CI.
Run `node --test tests/interfaces/layout/repository.test.mjs` after every change,
then the affected suites. Deliberate layout changes update docs/architecture.md
and its guard together.

Bug reports need redacted requests, runtime/source pins, typed outcomes and receipts.
Report vulnerabilities privately using [SECURITY.md](SECURITY.md), not public issues.
Never upload private payloads, account data or keys automatically. Publication,
public releases and upstream changes require separate authorization.

GitHub CI is configured for pushes and pull requests. Before accepting public PRs,
maintainers must require `offline-checks` and all three `portable-sdk` matrix jobs
on the default branch through repository rules/branch protection; the workflow
alone does not prevent merging. See the release checklist in [validation](docs/validation.md).

Unless explicitly stated otherwise, contributions to owned code are submitted
under [Apache-2.0](LICENSE). Preserve third-party copyright and license notices;
do not relicense imported code as project-owned code.
