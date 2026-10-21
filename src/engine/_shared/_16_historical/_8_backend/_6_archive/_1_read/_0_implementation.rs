use super::*;
use std::sync::Arc;

/// What the backend reads of the fork's history; the verified session is the real one.
pub trait Archive {
    /// The boundary block as archived, its slot, and the fork's position in it.
    fn boundary_block(&self) -> Result<(Arc<Value>, u64, u64), Error>;
    /// A finalized archive read (a transaction, an address's signatures, a slot range's blocks).
    fn archive(&mut self, query: &Value) -> Result<Value, Error>;
    /// An archived block by slot.
    fn archived_block(&mut self, slot: u64) -> Result<Value, Error>;
}

impl Archive for VerifiedSession {
    fn boundary_block(&self) -> Result<(Arc<Value>, u64, u64), Error> {
        VerifiedSession::boundary_block(self)
    }
    fn archive(&mut self, query: &Value) -> Result<Value, Error> {
        VerifiedSession::archive(self, query)
    }
    fn archived_block(&mut self, slot: u64) -> Result<Value, Error> {
        VerifiedSession::archived_block(self, slot)
    }
}

/// Finalized history before the fork's position: the boundary block's transactions before
/// it, then earlier archive reads; nothing at or after the position ever.
pub(in super::super::super) fn read(
    session: &mut impl Archive,
    query: ArchiveQuery,
) -> Result<Value, Error> {
    let (block, slot, position) = session.boundary_block()?;
    let block: &Value = &block;
    let boundary = transactions(block)?;
    let position = usize::try_from(position)
        .ok()
        .filter(|position| *position <= boundary.len())
        .ok_or_else(|| integrity("fork position outside its boundary block"))?;
    match query {
        ArchiveQuery::BoundaryBlock => Ok(json!({"slot":slot,"parentSlot":block["parentSlot"],
            "previousBlockhash":block["previousBlockhash"],"blockHeight":block["blockHeight"],
            "blockTime":block["blockTime"],"transactionIndex":position,
            "transactions":records(block, slot, position)?})),
        ArchiveQuery::Transaction {
            signature: wanted,
            history,
        } => {
            for (index, entry) in boundary.iter().enumerate() {
                if signature(entry)? == wanted {
                    // At or after the position, it did not happen on this fork.
                    return match index < position {
                        true => record(entry, slot, &block["blockTime"], Some(index as u64)),
                        false => Ok(Value::Null),
                    };
                }
            }
            if !history {
                return Ok(Value::Null);
            }
            let found = session.archive(&json!({"kind":"transaction","signature":wanted}))?;
            if found.is_null() {
                return Ok(Value::Null);
            }
            let at = found["slot"]
                .as_u64()
                .ok_or_else(|| integrity("archived transaction slot missing"))?;
            if signature(&found)? != wanted || at == slot {
                return Err(integrity(
                    "archived transaction differs from the boundary block",
                ));
            }
            // The provider's own index of the transaction in its block, when it reports one.
            let index = match &found["transactionIndex"] {
                Value::Null => None,
                index => Some(
                    index
                        .as_u64()
                        .ok_or_else(|| integrity("archived transaction index"))?,
                ),
            };
            match at < slot {
                true => record(&found, at, &found["blockTime"], index),
                false => Ok(Value::Null),
            }
        }
        ArchiveQuery::Block { slot: wanted } => {
            if wanted >= slot {
                return Err(Error::new(
                    "INVALID_QUERY",
                    "archived blocks precede the boundary slot",
                ));
            }
            let mut archived = session.archived_block(wanted)?;
            let count = transactions(&archived)?.len();
            archived["transactions"] = json!(records(&archived, wanted, count)?);
            archived["slot"] = json!(wanted);
            Ok(archived)
        }
        ArchiveQuery::Signatures {
            address,
            before,
            until,
            limit,
        } => signatures(
            session,
            block,
            slot,
            position,
            (address, before, until, limit),
        ),
        ArchiveQuery::Blocks { start, end } => {
            if start >= slot || end < start {
                return Ok(json!([]));
            }
            let end = end.min(slot - 1);
            let slots = session.archive(&json!({"kind":"blocks","start":start,"end":end}))?;
            Ok(json!(ascending(&slots, start, end)?))
        }
        ArchiveQuery::TokenAccountsByOwner {
            owner,
            mint,
            program_id,
        } => owner_candidates(session, block, position, (owner, mint, program_id)),
        ArchiveQuery::TokenHolders { mint, limit } => {
            holders(session, block, position, (mint, limit))
        }
        ArchiveQuery::TransactionsForAddress {
            address,
            sort_order,
            token_accounts,
            status,
            limit,
            pagination_token,
            slot: slots,
            block_time,
        } => {
            let query = AddressQuery {
                address,
                order: sort_order,
                token_accounts,
                status,
                limit,
                token: pagination_token,
                slot: slots,
                block_time,
            };
            address_history(session, block, slot, position, query)
        }
    }
}
