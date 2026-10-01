/** Test-only admission policy, read before execution from preserved evidence. */
import { readFile } from "node:fs/promises";
import { createHash } from "node:crypto";

export function policyFromVerification(prior: any): "STRICT" | "ARCHIVED_COMPUTE_METER_WARNING" {
  if (prior?.status !== "PASS") throw new Error("prior verification is not successful");
  if (prior.schema === "svm-simulate-m6-verification/v1") return "STRICT";
  if (prior.schema !== "svm-simulate-m6-verification/v2" || !Array.isArray(prior.metadataDivergences)) {
    throw new Error("unknown prior verification contract");
  }
  if (prior.metadataDivergences.length === 0) return "STRICT";
  if (prior.metadataPolicy !== "ARCHIVED_COMPUTE_METER_WARNING") throw new Error("prior divergences lack declared warning policy");
  return "ARCHIVED_COMPUTE_METER_WARNING";
}

export async function frozenPolicy(row: any) {
  const bytes = await readFile(row.frozenReceiptPath);
  const receipt = JSON.parse(bytes.toString());
  const cases = receipt.cases.filter((entry: any) => entry.caseId === row.candidate.id);
  if (cases.length !== 1) throw new Error("ambiguous/missing prior case");
  return { metadataPolicy: policyFromVerification(cases[0].verification),
    evidenceSha256: createHash("sha256").update(bytes).digest("hex") };
}
