# Development rules

## Layout
- All shipped implementation belongs in src/. Start at src/cli/main.rs for the
  executable or src/engine/mod.rs for the Rust API.
- Rust components telescope recursively: mod.rs, numbered child directories,
  _shared for genuinely shared services, and unnumbered tests.rs or tests/ for owned tests. Numbers
  describe workflow order where applicable and otherwise stable reading order.
- Routing directories have only mod.rs as production Rust source (main.rs for the CLI);
  an owned standalone tests.rs is allowed beside it.
  Every numbered child has mod.rs; a leaf may hold _0_implementation.rs beside its
  small entry. Use normal Rust module declarations, not production #[path]
  indirection. Preserve existing public API names through re-exports.
- Number leaf Rust implementation files too; only entry files and owned test
  files are exempt. The recursive hierarchy guard checks both files and folders.
- Aim for ~50 lines in module/entry files and ~200 in implementation/tests.
  The layout guard allows small overruns (70/250 physical lines), not large
  coordinators. Split by responsibility, never arbitrary numbered chunks.
  Keep public paths stable through explicit re-exports; inline tests belong in
  named test modules. Any exceptional ceiling needs a specific written reason.
- Keep main.rs as the only production Rust file directly under src/cli; CLI helpers belong in
  named component directories with mod.rs. The layout check enforces this.
- src/protocol and src/store are shared Rust crates; src/bindings/typescript is
  a thin client, not a second replay engine.
- A single owned test file is tests.rs beside its owner. Use tests/ only for multiple
  files or nested test helpers; never retain a folder wrapping just one Rust file.
  Do not create both tests.rs and tests/ for one owner. Root tests/ contains grouped
  cross-component checks. Each example has its own directory, README and entry point.
- Never number test directories; production step numbering does not include tests/.
- Public guides belong in docs/. Keep machine settings, operational logs,
  credentials, caches, generated evidence and private research outside Git.
- After EVERY change run:
  node --test tests/interfaces/layout/repository.test.mjs
  This is also a required CI check. Change layout rules only for a deliberate,
  documented architecture change, never merely to silence a failing check.

## Correctness
- Add a failing regression test before changing behavior.
- No silent current-state, runtime or default-value fallback.
- Keep historical baseline, overrides and results separate. Preserve original
  control verification, typed errors, cancellation and trace completeness.
- Synthetic tests do not establish historical parity. Never rebaseline evidence
  to turn a mismatch into a pass.
- Keep worker identities and source evidence pinned. Preserve failed attempts.
- The TypeScript binding must not duplicate Bank, provider or runtime logic.

## Validation and credentials
- Run the affected tests, cargo fmt --all --check, and the structure check.
- Normal tests are offline. Historical tests need explicit reviewed inputs.
  Live provider acquisition and expensive corpus runs require authorization.
- Alchemy credentials enter through API_ALCHEMY only when selected.
  Never put credentials in requests, examples, receipts or source files.
- Existing local credential storage must not be repurposed or exposed.
- Public releases, upstream submissions and external messages require approval.
- Report tested scope and remaining limitations. A partial replay is not exact
  coverage; COMPLETED does not mean the simulated transaction succeeded.
