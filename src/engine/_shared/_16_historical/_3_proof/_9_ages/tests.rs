use super::_0_implementation::walk;
use super::*;

/// The produced chain below a boundary at slot 100: slot s has blockhash "h{s}" and parent
/// s - 1 (slot 50 was skipped, so 51's parent is 49).
fn header(slot: u64) -> Value {
    let parent = if slot == 51 { 49 } else { slot - 1 };
    json!({"slot":slot,"blockhash":format!("h{slot}"),"previousBlockhash":format!("h{parent}"),
        "parentSlot":parent,"blockTime":1})
}

fn boundary() -> Value {
    json!({"parentSlot":99,"previousBlockhash":"h99","blockhash":"h100"})
}

#[test]
fn the_walk_links_parent_headers_from_the_boundary_and_stops_at_the_blockhash() {
    let mut read = Vec::new();
    let headers = walk(Vec::new(), &boundary(), "h96", |slot| {
        read.push(slot);
        Ok(header(slot))
    })
    .unwrap();
    // h99 is age 0 (the execution blockhash); h96 is the previous blockhash of slot 97.
    assert_eq!(read, [99, 98, 97]);
    assert_eq!(headers.len(), 3, "h96 has age 3");
    assert_eq!(headers[2]["previousBlockhash"], "h96");
    // A later walk continues below the proven headers, across a skipped slot.
    let mut later = Vec::new();
    let longer = walk(headers, &boundary(), "h49", |slot| {
        later.push(slot);
        Ok(header(slot))
    })
    .unwrap();
    assert_eq!(later.first(), Some(&96));
    assert_eq!(longer.last().unwrap()["slot"], 51, "51's parent is 49");
}

#[test]
fn an_absent_blockhash_walks_every_processing_age_and_no_further() {
    let headers = walk(Vec::new(), &boundary(), "never", |slot| match slot {
        0 => Err(Error::new("SOURCE_TRANSPORT", "below genesis")),
        slot => Ok(header(slot)),
    });
    let error = headers.unwrap_err();
    assert_eq!(
        error.code, "SOURCE_TRANSPORT",
        "the synthetic chain ends before 150"
    );
    let deep = |slot: u64| {
        json!({"slot":slot,"blockhash":format!("d{slot}"),"previousBlockhash":format!("d{}", slot - 1),
            "parentSlot":slot - 1})
    };
    let start = json!({"parentSlot":999,"previousBlockhash":"d999"});
    let all = walk(Vec::new(), &start, "never", |slot| Ok(deep(slot))).unwrap();
    assert_eq!(all.len(), 150);
    let done = walk(all, &start, "never", |slot| Ok(deep(slot))).unwrap_err();
    assert_eq!(done.code, "SESSION_PROTOCOL", "nothing is left to prove");
}

#[test]
fn a_header_that_does_not_link_or_cannot_be_read_fails_typed_and_named() {
    let unlinked = walk(Vec::new(), &boundary(), "h90", |slot| {
        let mut forged = header(slot);
        if slot == 98 {
            forged["blockhash"] = json!("other");
        }
        Ok(forged)
    })
    .unwrap_err();
    assert_eq!(unlinked.code, "SOURCE_INTEGRITY");
    assert!(
        unlinked.message.contains("archived block header 98"),
        "{}",
        unlinked.message
    );
    let backwards = walk(Vec::new(), &boundary(), "h90", |slot| {
        let mut forged = header(slot);
        forged["parentSlot"] = json!(slot);
        Ok(forged)
    })
    .unwrap_err();
    assert_eq!(backwards.code, "SOURCE_INTEGRITY");
    let offline = walk(Vec::new(), &boundary(), "h90", |slot| match slot {
        97 => Err(Error::new("SOURCE_TRANSPORT", "provider unreachable")),
        slot => Ok(header(slot)),
    })
    .unwrap_err();
    assert_eq!(offline.code, "SOURCE_TRANSPORT");
    assert!(
        offline
            .message
            .contains("block header 97, age 3 of the boundary queue, for recent blockhash h90"),
        "{}",
        offline.message
    );
    assert_eq!(
        offline.details.unwrap()["blockhashProof"],
        json!({"blockHeader":97,"queueAge":3,"recentBlockhash":"h90"})
    );
}
