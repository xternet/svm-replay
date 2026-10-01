use super::*;

pub fn assert_program_migration_bindings(fixture: &Value) -> Result<()> {
    let validate = || -> Result<()> {
        let runtime = field(fixture, "runtime")?;
        let Some(proofs) = runtime.get("programMigrationProofs") else {
            return Ok(());
        };
        for proof in array(proofs)? {
            let mut basis = object(proof)?.clone();
            let observed = basis
                .remove("proofHash")
                .ok_or_else(|| fail("proof hash missing"))?;
            check(
                observed
                    == json!(hash(format!(
                        "program-migration-initialization/v1\n{}\n",
                        canonical_json(&Value::Object(basis))
                    ))),
                "proof hash",
            )?;
            check(
                field(proof, "schema")? == "svm-program-migration-initialization/v1"
                    && (field(proof, "evidenceKind")? == "provider-attested"
                        || field(proof, "evidenceKind")? == "controlled-bank"),
                "proof schema/kind",
            )?;
            let migration = MIGRATIONS
                .iter()
                .find(|m| proof.get("featureId") == Some(&json!(m.feature)))
                .ok_or_else(|| fail("migration account identity"))?;
            check(
                field(proof, "program")? == migration.program
                    && field(proof, "programData")? == migration.data
                    && field(proof, "buffer")? == migration.buffer,
                "migration account identity",
            )?;
            let mut expected_keys = migration.keys();
            expected_keys.sort();
            for name in ["parentAccountHashes", "targetAccountHashes"] {
                let values = array(field(proof, name)?)?;
                check(values.len() == 3, "exact proof account hashes")?;
                for (value, id) in values.iter().zip(expected_keys) {
                    check(field(value, "pubkey")? == id, "exact proof account hashes")?;
                    digest(string(field(value, "sha256")?)?)?;
                }
            }
            let target = field(fixture, "target")?;
            let binding = field(runtime, "binding")?;
            let activation = integer(field(proof, "activationSlot")?)?;
            let parent = integer(field(proof, "parentSlot")?)?;
            let slot = integer(field(proof, "targetSlot")?)?;
            check(
                field(target, "targetSlot")? == &json!(slot)
                    && field(target, "parentSlot")? == &json!(parent)
                    && activation > parent
                    && activation <= slot
                    && field(proof, "blockSourceHash")?
                        == field(field(runtime, "bankContext")?, "blockSourceHash")?
                    && field(proof, "executorSourceId")?
                        == field(field(binding, "executor")?, "id")?
                    && array(field(binding, "features")?)?.iter().any(|feature| {
                        feature.get("id") == proof.get("featureId")
                            && feature.get("activationSlot") == Some(&json!(activation))
                    }),
                "initialized runtime boundary",
            )?;
            let accounts = array(field(fixture, "accounts")?)?;
            for expected in array(field(proof, "targetAccountHashes")?)? {
                let account = accounts
                    .iter()
                    .find(|v| v.get("pubkey") == expected.get("pubkey"))
                    .ok_or_else(|| fail("initialized account missing"))?;
                check(
                    field(account, "sourceSlot")? == &json!(slot)
                        && field(expected, "sha256")? == &json!(hash(canonical_json(account))),
                    "initialized account changed",
                )?;
            }
            let rent = accounts
                .iter()
                .find(|v| v.get("pubkey") == Some(&json!(RENT)))
                .ok_or_else(|| fail("initialized Rent missing"))?;
            check(
                field(proof, "rentAccountHash")? == &json!(hash(canonical_json(rent))),
                "initialized Rent changed",
            )?;
        }
        Ok(())
    };
    validate().map_err(error)
}
