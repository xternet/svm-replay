use super::*;

impl<'a> Recovery<'a> {
    pub(in super::super::super) fn new(
        a: &'a Analysis,
        transactions: &[dependencies::SemanticTransaction],
        binding: &'a Value,
        block: &'a Value,
    ) -> Self {
        Self {
            a,
            block,
            binding,
            programs: transactions
                .iter()
                .take(a.index as usize + 1)
                .flat_map(|t| t.program_ids.iter().cloned())
                .collect(),
            images: BTreeMap::new(),
            proofs: Vec::new(),
        }
    }
    pub(in super::super::super) fn links(
        &mut self,
        history: &mut History<'_>,
        keys: &[String],
    ) -> Result<BTreeMap<String, Option<String>>, Error> {
        let mut links = BTreeMap::new();
        for key in keys {
            if [CLOCK, INSTRUCTIONS, RECENT_BLOCKHASHES].contains(&key.as_str()) {
                if links.insert(key.clone(), None).is_some() {
                    return Err(invalid("duplicate program link"));
                }
                continue;
            }
            let mut account = match self.images.get(key) {
                Some(image) => image.clone(),
                None => history.account(key, self.a.parent, &Roles::default())?,
            };
            if account["presence"] == "absent"
                && self.programs.contains(key)
                && preexisting(self.block, key)?
            {
                if let Some(result) = recover(self, history, key)? {
                    account = result["account"].clone();
                    self.images.insert(key.clone(), account.clone());
                    self.proofs.push(result["proof"].clone());
                }
            }
            if links
                .insert(key.clone(), program_data_address(&account)?)
                .is_some()
            {
                return Err(invalid("duplicate program link"));
            }
        }
        Ok(links)
    }
    pub(in super::super::super) fn apply(&self, accounts: &mut [Value]) -> Result<(), Error> {
        for account in accounts {
            let key = account["pubkey"]
                .as_str()
                .ok_or_else(|| invalid("parent key missing"))?;
            if let Some(image) = self.images.get(key) {
                if account["presence"] != "absent" || account["sourceSlot"] != self.a.parent {
                    return Err(Error::new(
                        "SOURCE_INTEGRITY",
                        "recovered header conflicts with parent image",
                    ));
                }
                *account = image.clone();
            }
        }
        Ok(())
    }
}
