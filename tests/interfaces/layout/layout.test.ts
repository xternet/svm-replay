import { test, expect } from "bun:test";
import { existsSync, readdirSync, readFileSync } from "node:fs";
import { resolve } from "node:path";

const root = resolve(import.meta.dir, "../../..");
test("one repository exposes the engine, grouped tests and current docs", () => {
  for (const path of ["src/cli/main.rs", "src/engine/mod.rs", "src/engine/_0_validate/mod.rs",
    "src/engine/_1_resolve_runtime/mod.rs", "src/engine/_2_prepare_state/mod.rs",
    "src/engine/_3_verify_original/mod.rs", "src/engine/_4_simulate/mod.rs",
    "src/engine/_5_finalize/mod.rs", "src/protocol/Cargo.toml", "src/store/Cargo.toml",
    "tests/README.md", "docs/usage.md", "docs/architecture.md", "docs/limitations.md",
    "docs/validation.md"]) expect(existsSync(resolve(root, path))).toBe(true);
  expect(existsSync(resolve(root, "crates"))).toBe(false);
  expect(readdirSync(resolve(root, "tests")).filter(p => /\.(rs|ts)$/.test(p))).toEqual([]);
  expect(readdirSync(resolve(root, "docs")).filter(p => p.startsWith("m17-"))).toEqual([]);
  expect(readFileSync(resolve(root, "src/engine/mod.rs"), "utf8").split("\n").length).toBeLessThan(180);
});
