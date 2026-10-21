use super::*;

/// The account keys each branch's fork holds, kept as the worker keeps them: the boundary
/// fixture's keys, then what hydration loaded and what overrides defined on that branch.
/// It lists a fork's loaded accounts; it is not an index of every account at the slot.
pub struct Inventory {
    base: BTreeSet<String>,
    branches: BTreeMap<u64, BTreeSet<String>>,
}

impl Inventory {
    pub fn new(base: Vec<String>) -> Self {
        let base: BTreeSet<String> = base.into_iter().collect();
        Self {
            branches: BTreeMap::from([(0, base.clone())]),
            base,
        }
    }
    pub fn fork(&mut self, from: u64, to: u64) -> Result<(), Error> {
        let keys = self.branch(from)?.clone();
        self.branches.insert(to, keys);
        Ok(())
    }
    pub fn close(&mut self, branch: u64) {
        self.branches.remove(&branch);
    }
    pub fn reset(&mut self, branch: u64) {
        self.branches.insert(branch, self.base.clone());
    }
    pub fn add(
        &mut self,
        branch: u64,
        keys: impl IntoIterator<Item = String>,
    ) -> Result<(), Error> {
        let missing = || {
            Error::new(
                "SESSION_INTEGRITY",
                format!("branch {branch} has no inventory"),
            )
        };
        self.branches
            .get_mut(&branch)
            .ok_or_else(missing)?
            .extend(keys);
        Ok(())
    }
    pub fn keys(&self, branch: u64) -> Result<Vec<String>, Error> {
        Ok(self.branch(branch)?.iter().cloned().collect())
    }
    fn branch(&self, branch: u64) -> Result<&BTreeSet<String>, Error> {
        self.branches.get(&branch).ok_or_else(|| {
            Error::new(
                "SESSION_INTEGRITY",
                format!("branch {branch} has no inventory"),
            )
        })
    }
}

/// The keys a command defines on its branch when it succeeds.
pub fn defined_keys(command: &SessionCommand) -> Vec<String> {
    match command {
        SessionCommand::OverrideAccounts { entries } => {
            entries.iter().map(|entry| entry.pubkey.clone()).collect()
        }
        SessionCommand::SetBalance { pubkey, .. } => vec![pubkey.clone()],
        _ => Vec::new(),
    }
}

/// Every account `branch` has loaded, with its image (`null` for a proven absence),
/// read without acquiring anything.
pub fn known_accounts(
    branches: &mut Branches,
    session: &mut impl Commands,
    branch: u64,
    budget: &ExecutionBudget,
) -> Result<Value, Error> {
    let keys = branches.inventory.keys(branch)?;
    branches.enter(session, branch, budget)?;
    let mut accounts = Vec::with_capacity(keys.len());
    for chunk in keys.chunks(100) {
        let entries = session.peek(chunk.to_vec(), budget)?;
        let entries = entries
            .as_array()
            .filter(|entries| entries.len() == chunk.len())
            .ok_or_else(|| Error::new("SESSION_PROTOCOL", "peek reply does not match"))?;
        for (key, entry) in chunk.iter().zip(entries) {
            if entry["loaded"] != true {
                return Err(Error::new(
                    "SESSION_INTEGRITY",
                    format!("branch {branch} lists {key}, which its fork has not loaded"),
                ));
            }
            accounts.push(json!({"pubkey": key, "account": entry["account"]}));
        }
    }
    Ok(Value::Array(accounts))
}
