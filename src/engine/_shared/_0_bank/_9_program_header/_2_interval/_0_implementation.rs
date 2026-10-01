use super::*;

pub(in super::super) fn review(registry: &Value, start: u64, end: u64) -> Result<()> {
    let features = array(&registry["features"])?;
    let mut seen = BTreeSet::new();
    for feature in features {
        check(
            seen.insert(pubkey(&feature["id"])?),
            "duplicate lifecycle feature",
        )?;
        if !feature["activationSlot"].is_null() {
            integer(&feature["activationSlot"])?;
        }
    }
    let activation = |key: &str| -> Result<Option<u64>> {
        let row = features
            .iter()
            .find(|f| f["id"] == key)
            .ok_or_else(|| fail("missing lifecycle feature"))?;
        if row["activationSlot"].is_null() {
            Ok(None)
        } else {
            Ok(Some(integer(&row["activationSlot"])?))
        }
    };
    check(
        activation("2aQJYqER2aKyb3cZw22v4SL2xMX7vwXBRWfvS4pTrtED")?.is_none_or(|s| s > end),
        "loader migration enabled",
    )?;
    check(
        activation("CJzY83ggJHqPGDq8VisV3U91jDJLuEaALZooBrXtnnLU")?.is_some_and(|s| s <= start),
        "rent normalization not active",
    )?;
    let profiles = array(&registry["profiles"])?;
    let first = profiles
        .first()
        .ok_or_else(|| fail("missing lifecycle profiles"))?;
    let earliest = integer(&first["earliestSlot"])?;
    let mut cursor = start;
    if start < earliest {
        // The execution registry begins at a sampled boundary. Earlier native
        // checks may be used as evidence only within the identical feature era;
        // this does NOT extend execution-profile selection to earlier slots.
        check(
            first["executorSourceId"] == REVIEWED[0],
            "unreviewed earlier lifecycle",
        )?;
        check(
            !features.iter().any(|f| {
                f["activationSlot"]
                    .as_u64()
                    .is_some_and(|s| s > start && s <= earliest)
            }),
            "earlier witness crosses feature activation",
        )?;
        cursor = earliest;
    }
    for profile in profiles {
        let last = integer(&profile["latestSlot"])?;
        if last < cursor {
            continue;
        }
        check(
            integer(&profile["earliestSlot"])? <= cursor,
            "lifecycle runtime gap",
        )?;
        check(
            REVIEWED.contains(&string(&profile["executorSourceId"])?),
            "unreviewed lifecycle runtime",
        )?;
        if last >= end {
            return Ok(());
        }
        cursor = last
            .checked_add(1)
            .ok_or_else(|| fail("lifecycle slot overflow"))?;
    }
    Err(fail("incomplete lifecycle interval"))
}
