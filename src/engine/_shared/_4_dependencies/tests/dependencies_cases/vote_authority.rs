use super::*;

const VOTE: &str = "Vote111111111111111111111111111111111111111";

fn vote(opcode: u32) -> Value {
    raw(
        "vote",
        &["authority", "vote-state", VOTE],
        1,
        json!([{"programIdIndex":2,"accounts":[1,0],
            "data":bs58::encode(opcode.to_le_bytes()).into_string()}]),
    )
}

#[test]
fn readonly_vote_authority_does_not_select_a_vote_for_its_fee_debit() {
    for opcode in [8, 9, 12, 13, 14, 15] {
        let writer = summary(&vote(opcode), 0);
        assert!(!writer
            .application_writable_accounts
            .contains(&"authority".into()));
        let target = summary(&simple("target", "payer", "authority", SYSTEM), 1);
        assert!(
            compute_backward_semantic_closure(&[writer.clone(), target], 1, &[])
                .unwrap()
                .selected_indices
                .is_empty()
        );
        let vote_reader = summary(&simple("target", "payer", "vote-state", "reader"), 1);
        assert_eq!(
            compute_backward_semantic_closure(&[writer, vote_reader], 1, &[])
                .unwrap()
                .selected_indices,
            vec![0]
        );
    }
}

#[test]
fn vote_with_other_instruction_or_withdraw_is_not_assumed_readonly() {
    let withdraw = summary(&vote(3), 0);
    assert!(withdraw
        .application_writable_accounts
        .contains(&"authority".into()));
    let mut multi = vote(14);
    multi["transaction"]["message"]["instructions"]
        .as_array_mut()
        .unwrap()
        .push(json!({"programIdIndex":2,"accounts":[0],"data":""}));
    assert!(summary(&multi, 0)
        .application_writable_accounts
        .contains(&"authority".into()));
    let mut same = vote(14);
    same["transaction"]["message"]["instructions"][0]["accounts"] = json!([0, 0]);
    assert!(summary(&same, 0)
        .application_writable_accounts
        .contains(&"authority".into()));
}
