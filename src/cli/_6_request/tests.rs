use super::*;

#[test]
fn boundary_requires_unique_signature_and_a_full_block() {
    let signature = "1".repeat(64);
    let binding = json!({"targetSlot":20,"runtimeProfileId":"profile",
        "executor":{"id":"executor","litesvmCommit":"commit"},"features":[]});
    let tx = json!({"transaction":{"signatures":[signature]}});
    let raw = serde_json::to_vec(&json!({"result":{"parentSlot":19,"transactions":[tx]}})).unwrap();
    let candidate = boundary(&raw, &signature, &binding).unwrap();
    assert_eq!(candidate["transactionIndex"], 0);
    assert_eq!(candidate["id"], format!("tx-{signature}"));
    assert_eq!(candidate["blockSourceHash"], json!(Digest::of(&raw)));
    assert_eq!(
        candidate["rawEvidenceHash"],
        json!(Digest::of(format!(
            "{}\n0\n{signature}\n",
            Digest::of(&raw).as_str()
        )))
    );
    for bad in [
        json!({"error":{},"result":{"transactions":[tx]}}),
        json!({"result":{"parentSlot":19,"transactions":[]}}),
        json!({"result":{"parentSlot":19,"transactions":[tx,tx]}}),
        json!({"result":{"parentSlot":20,"transactions":[tx]}}),
        json!({"result":{"parentSlot":19,"transactions":[{}]}}),
    ] {
        assert!(boundary(&serde_json::to_vec(&bad).unwrap(), &signature, &binding).is_err());
    }
    assert!(boundary(&raw, "invalid signature", &binding).is_err());
}
