use super::*;

mod _0_implementation;

pub(in super::super) use _0_implementation::finish;

fn decorate_runtime(
    runtime: &mut Value,
    after_slot: Option<u64>,
    recent: Vec<Value>,
    stake_proofs: Vec<Value>,
    snapshot: Option<Value>,
    boundary: &Boundary<'_>,
) -> Result<(), Error> {
    if let Some(after) = after_slot {
        runtime["afterSlot"] = json!(after);
    }
    for (name, values) in [
        ("recentAddressTables", recent),
        ("stakeInitializationProofs", stake_proofs),
        (
            "programMigrationProofs",
            array(&boundary.migrations["proofs"], "migration proofs")?.clone(),
        ),
    ] {
        if !values.is_empty() {
            runtime[name] = json!(values);
        }
    }
    if let Some(value) = snapshot {
        runtime["initializedStakeSnapshot"] = value;
    }
    if !boundary.credit_proofs.is_empty() {
        runtime["systemCreditPreloads"] = json!(boundary.credit_proofs);
    }
    if !boundary.header_proofs.is_empty() {
        runtime["programHeaderProofs"] = json!(boundary.header_proofs);
    }
    Ok(())
}
