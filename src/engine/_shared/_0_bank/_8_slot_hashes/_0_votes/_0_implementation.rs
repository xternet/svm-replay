use super::*;
use bincode::Options;
use solana_vote_interface::instruction::VoteInstruction;

pub(in super::super) fn witnesses(block: &Value, observer: u64) -> Result<Vec<(u64, String)>> {
    let mut found = Vec::new();
    for row in array(field(block, "transactions")?)? {
        let meta = field(row, "meta")?;
        if !field(meta, "err")?.is_null() {
            continue;
        }
        let message = field(field(row, "transaction")?, "message")?;
        let mut keys = array(field(message, "accountKeys")?)?
            .iter()
            .collect::<Vec<_>>();
        if let Some(loaded) = meta.get("loadedAddresses") {
            keys.extend(array(field(loaded, "writable")?)?);
            keys.extend(array(field(loaded, "readonly")?)?);
        }
        for instruction in array(field(message, "instructions")?)? {
            let index = usize::try_from(integer(field(instruction, "programIdIndex")?)?)
                .map_err(|_| fail("program index overflow"))?;
            let key = keys
                .get(index)
                .ok_or_else(|| fail("program index outside keys"))?;
            if string(key)? != "Vote111111111111111111111111111111111111111" {
                continue;
            }
            let data = bs58::decode(string(field(instruction, "data")?)?)
                .into_vec()
                .map_err(|e| fail(format!("vote data: {e}")))?;
            check(
                data.len() <= 1232 && data.len() >= 4,
                "vote instruction size",
            )?;
            let tag = u32::from_le_bytes(data[..4].try_into().map_err(|_| fail("vote tag"))?);
            // Non-voting authority/commission instructions cannot attest a Bank hash.
            if !matches!(tag, 2 | 6 | 8 | 9 | 12 | 13 | 14 | 15) {
                continue;
            }
            let vote: VoteInstruction = bincode::DefaultOptions::new()
                .with_fixint_encoding()
                .with_limit(1232)
                .allow_trailing_bytes()
                .deserialize(&data)
                .map_err(|e| fail(format!("native vote decoding: {e}")))?;
            if let VoteInstruction::Vote(v) | VoteInstruction::VoteSwitch(v, _) = &vote {
                check(
                    v.slots.windows(2).all(|s| s[0] < s[1]),
                    "unordered legacy vote slots",
                )?;
            }
            let slot = vote
                .last_voted_slot()
                .ok_or_else(|| fail("successful vote has no slots"))?;
            check(
                slot < observer,
                "successful vote is not for an earlier slot",
            )?;
            found.push((slot, vote.hash().to_string()));
        }
    }
    Ok(found)
}
