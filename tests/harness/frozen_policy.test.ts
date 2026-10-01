import { expect, test } from "bun:test";
import { policyFromVerification } from "./frozen_policy";

test("freeze policy from prior successful evidence, never a new failure", () => {
  expect(policyFromVerification({ schema: "svm-simulate-m6-verification/v1", status: "PASS" })).toBe("STRICT");
  const prior = { schema: "svm-simulate-m6-verification/v2", status: "PASS", metadataPolicy: "ARCHIVED_COMPUTE_METER_WARNING", metadataDivergences: [] };
  expect(policyFromVerification(prior)).toBe("STRICT");
  expect(policyFromVerification({ ...prior, metadataDivergences: [{ field: "computeUnits" }] })).toBe("ARCHIVED_COMPUTE_METER_WARNING");
  expect(() => policyFromVerification({ ...prior, status: "FAIL" })).toThrow();
  expect(() => policyFromVerification({ ...prior, metadataDivergences: undefined })).toThrow();
  expect(() => policyFromVerification({ ...prior, schema: "unknown" })).toThrow();
});
