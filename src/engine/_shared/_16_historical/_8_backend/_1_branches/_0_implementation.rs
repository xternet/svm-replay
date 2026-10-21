use super::*;
use std::collections::{BTreeMap, BTreeSet};

/// The session operations the backend needs; the verified session is the real one.
pub trait Commands {
    fn command(
        &mut self,
        command: SessionCommand,
        budget: &ExecutionBudget,
    ) -> Result<Value, Error>;
    /// The active fork's images of `pubkeys` without acquiring any (see `peek_accounts`).
    fn peek(&mut self, pubkeys: Vec<String>, budget: &ExecutionBudget) -> Result<Value, Error>;
    /// The keys the boundary fork holds.
    fn base_keys(&self) -> Vec<String>;
    /// The keys hydration loaded into the active fork since the last call.
    fn take_hydrated_keys(&mut self) -> Vec<String>;
}

impl Commands for VerifiedSession {
    fn command(
        &mut self,
        command: SessionCommand,
        budget: &ExecutionBudget,
    ) -> Result<Value, Error> {
        self.request(command, budget)
    }

    fn peek(&mut self, pubkeys: Vec<String>, budget: &ExecutionBudget) -> Result<Value, Error> {
        self.peek_accounts(pubkeys, budget)
    }

    fn base_keys(&self) -> Vec<String> {
        VerifiedSession::base_keys(self).to_vec()
    }

    fn take_hydrated_keys(&mut self) -> Vec<String> {
        VerifiedSession::take_hydrated_keys(self)
    }
}

/// Isolated branch handles over the worker's single active fork: every request names
/// its branch, and the backend selects it first, so interleaved handles never mix.
/// Each branch counts its state changes; a fork remembers its parent's count, so a
/// commit back into the parent is refused once the parent changed meanwhile.
pub struct Branches {
    active: u64,
    live: BTreeSet<u64>,
    revisions: BTreeMap<u64, u64>,
    parents: BTreeMap<u64, (u64, u64)>,
    pub(in super::super) inventory: super::super::_5_inventory::Inventory,
}

impl Branches {
    pub fn new(base_keys: Vec<String>) -> Self {
        Self {
            active: 0,
            live: BTreeSet::from([0]),
            revisions: BTreeMap::from([(0, 0)]),
            parents: BTreeMap::new(),
            inventory: super::super::_5_inventory::Inventory::new(base_keys),
        }
    }

    pub fn revision(&self, branch: u64) -> Result<u64, Error> {
        self.revisions
            .get(&branch)
            .copied()
            .ok_or_else(|| Error::new("UNKNOWN_BRANCH", format!("branch {branch} is not open")))
    }

    pub fn enter(
        &mut self,
        session: &mut impl Commands,
        branch: u64,
        budget: &ExecutionBudget,
    ) -> Result<(), Error> {
        if !self.live.contains(&branch) {
            return Err(Error::new(
                "UNKNOWN_BRANCH",
                format!("branch {branch} is not open"),
            ));
        }
        if self.active != branch {
            session.command(SessionCommand::Select { fork_id: branch }, budget)?;
            self.active = branch;
        }
        Ok(())
    }

    pub fn fork(
        &mut self,
        session: &mut impl Commands,
        branch: u64,
        budget: &ExecutionBudget,
    ) -> Result<u64, Error> {
        self.enter(session, branch, budget)?;
        let reply = session.command(SessionCommand::Snapshot, budget)?;
        let id = reply["forkId"]
            .as_u64()
            .ok_or_else(|| Error::new("SESSION_PROTOCOL", "snapshot fork id missing"))?;
        if !self.live.insert(id) {
            return Err(Error::new(
                "SESSION_PROTOCOL",
                format!("snapshot reused open branch {id}"),
            ));
        }
        let revision = self.revision(branch)?;
        self.revisions.insert(id, 0);
        self.parents.insert(id, (branch, revision));
        self.inventory.fork(branch, id)?;
        Ok(id)
    }

    /// Replaces `into` with `branch`, a fork of it, unless `into` changed since the fork.
    /// `into` is discarded; its handle continues as `branch`.
    pub fn commit(
        &mut self,
        session: &mut impl Commands,
        branch: u64,
        into: u64,
        budget: &ExecutionBudget,
    ) -> Result<(), Error> {
        let current = self.revision(into)?;
        match self.parents.get(&branch) {
            Some((parent, revision)) if *parent == into && *revision == current => {}
            Some((parent, revision)) if *parent == into => {
                return Err(Error::new(
                    "STALE_BRANCH",
                    format!("branch {into} changed since branch {branch} was forked"),
                )
                .with_details(json!({"forkedAtRevision":revision,"currentRevision":current})))
            }
            _ => {
                return Err(Error::new(
                    "INVALID_COMMIT",
                    format!("branch {branch} is not a fork of branch {into}"),
                ))
            }
        }
        self.enter(session, branch, budget)?;
        self.close(session, into, budget)?;
        // Other forks of the replaced branch can no longer commit anywhere.
        self.parents.remove(&branch);
        Ok(())
    }

    pub fn close(
        &mut self,
        session: &mut impl Commands,
        branch: u64,
        budget: &ExecutionBudget,
    ) -> Result<(), Error> {
        if !self.live.contains(&branch) {
            return Err(Error::new(
                "UNKNOWN_BRANCH",
                format!("branch {branch} is not open"),
            ));
        }
        if self.active == branch {
            let other = self
                .live
                .iter()
                .copied()
                .find(|id| *id != branch)
                .ok_or_else(|| {
                    Error::new("LAST_BRANCH", "the only open branch cannot be closed")
                })?;
            self.enter(session, other, budget)?;
        }
        session.command(SessionCommand::CloseFork { fork_id: branch }, budget)?;
        self.live.remove(&branch);
        self.revisions.remove(&branch);
        self.parents.remove(&branch);
        self.inventory.close(branch);
        Ok(())
    }

    /// Runs a worker command on `branch`. Fork management stays with the backend, so a
    /// client cannot move the worker's active fork underneath another handle.
    pub fn run(
        &mut self,
        session: &mut impl Commands,
        branch: u64,
        command: SessionCommand,
        budget: &ExecutionBudget,
    ) -> Result<Value, Error> {
        if matches!(
            command,
            SessionCommand::Snapshot
                | SessionCommand::Select { .. }
                | SessionCommand::CloseFork { .. }
                | SessionCommand::Close
                | SessionCommand::Hydrate { .. }
                | SessionCommand::ConfigureRpc { .. }
                | SessionCommand::ProveBlockhashQueue { .. }
        ) {
            return Err(Error::new(
                "INVALID_BACKEND_COMMAND",
                "fork, hydration, blockhash-proof and RPC-context commands are managed by the backend",
            ));
        }
        let changes = matches!(
            command,
            SessionCommand::SetBalance { .. }
                | SessionCommand::OverrideAccounts { .. }
                | SessionCommand::RpcSend { .. }
                | SessionCommand::Send { .. }
                | SessionCommand::Advance { .. }
                | SessionCommand::Reset
        );
        let defined = super::super::_5_inventory::defined_keys(&command);
        let resets = matches!(command, SessionCommand::Reset);
        self.enter(session, branch, budget)?;
        // A failed change may still have committed (for example a fee), so count it.
        let result = session.command(command, budget);
        // Hydration can load accounts even for a command that then fails.
        self.inventory.add(branch, session.take_hydrated_keys())?;
        if result.is_ok() {
            if resets {
                self.inventory.reset(branch);
            }
            self.inventory.add(branch, defined)?;
        }
        if changes {
            *self.revisions.get_mut(&branch).ok_or_else(|| {
                Error::new("UNKNOWN_BRANCH", format!("branch {branch} is not open"))
            })? += 1;
        }
        result
    }
}
