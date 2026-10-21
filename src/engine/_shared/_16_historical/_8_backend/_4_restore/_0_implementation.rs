use super::*;

/// Puts `pubkeys` on `branch` back to their boundary images, discarding local edits: the
/// images come from a disposable copy reset to the boundary (acquiring what the boundary
/// state needs), and a proven absence restores as an explicit absence.
pub fn restore(
    branches: &mut Branches,
    session: &mut impl Commands,
    branch: u64,
    pubkeys: Vec<String>,
    budget: &ExecutionBudget,
) -> Result<Value, Error> {
    if pubkeys.is_empty() || pubkeys.len() > 100 {
        return Err(Error::new(
            "INVALID_REQUEST",
            "restore 1 to 100 accounts at a time",
        ));
    }
    let copy = branches.fork(session, branch, budget)?;
    let images = boundary_images(branches, session, copy, &pubkeys, budget);
    let images = match (images, branches.close(session, copy, budget)) {
        (Ok(images), Ok(())) => images,
        (Err(error), Ok(())) | (Ok(_), Err(error)) => return Err(error),
        (Err(error), Err(cleanup)) => {
            return Err(super::super::_2_simulate::with_cleanup(error, cleanup))
        }
    };
    let entries = pubkeys
        .iter()
        .zip(&images)
        .map(|(pubkey, image)| override_entry(pubkey, image))
        .collect::<Result<Vec<_>, Error>>()?;
    branches.run(
        session,
        branch,
        SessionCommand::OverrideAccounts { entries },
        budget,
    )?;
    Ok(json!({"restored": images}))
}

fn boundary_images(
    branches: &mut Branches,
    session: &mut impl Commands,
    copy: u64,
    pubkeys: &[String],
    budget: &ExecutionBudget,
) -> Result<Vec<Value>, Error> {
    branches.run(session, copy, SessionCommand::Reset, budget)?;
    let reply = branches.run(
        session,
        copy,
        SessionCommand::Accounts {
            pubkeys: pubkeys.to_vec(),
        },
        budget,
    )?;
    reply
        .as_array()
        .filter(|images| images.len() == pubkeys.len())
        .cloned()
        .ok_or_else(|| Error::new("SESSION_PROTOCOL", "account images missing"))
}

fn override_entry(
    pubkey: &str,
    image: &Value,
) -> Result<svm_replay_protocol::StateOverride, Error> {
    let entry = match image {
        Value::Null => json!({"pubkey": pubkey, "lamports": "0"}),
        image => json!({"pubkey": pubkey, "lamports": image["lamports"], "owner": image["owner"],
            "executable": image["executable"], "rentEpoch": image["rentEpoch"],
            "dataBase64": image["dataBase64"]}),
    };
    serde_json::from_value(entry)
        .map_err(|e| Error::new("SESSION_PROTOCOL", format!("boundary image {pubkey}: {e}")))
}
