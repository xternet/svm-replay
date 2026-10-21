use super::*;
use crate::shared::runtime::CancellationToken;
use std::collections::BTreeMap;
use std::time::Duration;

/// A stand-in with the worker's fork semantics (one active fork; snapshot copies it,
/// select switches, close refuses the active one). It checks the backend's bookkeeping;
/// historical execution parity is tested against real workers elsewhere.
#[derive(Default)]
pub(super) struct Forks {
    pub(super) forks: BTreeMap<u64, BTreeMap<String, u64>>,
    pub(super) active: u64,
    pub(super) next: u64,
    pub(super) selects: Vec<u64>,
    pub(super) diverge: bool,
    pub(super) fail_close: bool,
    /// Keys the next command "hydrates", as the verified session reports them.
    pub(super) hydrate: Vec<String>,
    /// Journal side commands: recorded hydrations, or those a replay hands back.
    pub(super) recording: bool,
    pub(super) sides: Vec<Value>,
    pub(super) replay: std::collections::VecDeque<Value>,
    pub(super) replaying: bool,
}

/// Hydrations become side commands while a journal records, and a replay applies the
/// recorded ones instead of acquiring, as the verified session does.
impl Journaled for Forks {
    fn journal_pins(&self) -> Value {
        json!({"slot": 100, "receiptSha256": "r0"})
    }
    fn journal_input(&self) -> Value {
        json!({"receiptPath": "receipt.json"})
    }
    fn record_side_commands(&mut self) {
        self.recording = true;
    }
    fn take_side_commands(&mut self) -> Vec<Value> {
        std::mem::take(&mut self.sides)
    }
    fn replay_side_commands(&mut self, sides: Vec<Value>) {
        self.replaying = true;
        self.replay = sides.into();
    }
    fn finish_replay(&mut self) -> Result<(), Error> {
        self.replaying = false;
        match self.replay.is_empty() {
            true => Ok(()),
            false => Err(Error::new(
                "JOURNAL_INTEGRITY",
                "unused recorded hydrations",
            )),
        }
    }
    fn state_digest(&mut self, _: &ExecutionBudget) -> Result<String, Error> {
        Ok(json!({"active": self.active, "forks": self.forks}).to_string())
    }
}

impl Forks {
    pub(super) fn new() -> Self {
        Self {
            forks: BTreeMap::from([(0, BTreeMap::from([("a".into(), 10)]))]),
            next: 1,
            ..Self::default()
        }
    }
    fn current(&mut self) -> &mut BTreeMap<String, u64> {
        self.forks.get_mut(&self.active).unwrap()
    }
    fn output(&self, wire: &str) -> Value {
        // "credit:<key>:<lamports>" or "fail".
        let ok = wire != "fail";
        json!({"status": if ok {"ok"} else {"failed"},
            "normalizedError": if ok {Value::Null} else {json!("\"InvalidAccountData\"")},
            "computeUnits": 150, "logs": [format!("log {wire}")], "returnData": null,
            "fee": "5000", "accountTransitions": [wire]})
    }
}

/// Fork bookkeeping keeps no archive; archive reads have their own stand-in.
impl Archive for Forks {
    fn boundary_block(&self) -> Result<(std::sync::Arc<Value>, u64, u64), Error> {
        Err(Error::new("HISTORICAL_DATA_UNAVAILABLE", "no archive"))
    }
    fn archive(&mut self, _: &Value) -> Result<Value, Error> {
        Err(Error::new("HISTORICAL_DATA_UNAVAILABLE", "no archive"))
    }
    fn archived_block(&mut self, _: u64) -> Result<Value, Error> {
        Err(Error::new("HISTORICAL_DATA_UNAVAILABLE", "no archive"))
    }
}

impl Commands for Forks {
    fn base_keys(&self) -> Vec<String> {
        vec!["a".to_string()]
    }
    fn take_hydrated_keys(&mut self) -> Vec<String> {
        let keys: Vec<String> = match self.replaying {
            true => self
                .replay
                .pop_front()
                .map(|side| serde_json::from_value(side["keys"].clone()).unwrap())
                .unwrap_or_default(),
            false => std::mem::take(&mut self.hydrate),
        };
        if self.recording && !self.replaying && !keys.is_empty() {
            self.sides.push(json!({"kind": "hydrate", "keys": keys}));
        }
        for key in &keys {
            self.current().entry(key.clone()).or_insert(7);
        }
        keys
    }
    fn peek(&mut self, pubkeys: Vec<String>, _: &ExecutionBudget) -> Result<Value, Error> {
        let current = self.current().clone();
        Ok(json!(pubkeys.iter().map(|key| match current.get(key) {
            Some(lamports) => json!({"loaded": true, "account": {"pubkey": key, "lamports": lamports.to_string()}}),
            None => json!({"loaded": false}),
        }).collect::<Vec<_>>()))
    }

    fn command(&mut self, command: SessionCommand, _: &ExecutionBudget) -> Result<Value, Error> {
        Ok(match command {
            SessionCommand::Snapshot => {
                let copy = self.current().clone();
                self.forks.insert(self.next, copy);
                self.next += 1;
                json!({"forkId": self.next - 1})
            }
            SessionCommand::Select { fork_id } => {
                assert!(self.forks.contains_key(&fork_id));
                self.active = fork_id;
                self.selects.push(fork_id);
                json!({"forkId": fork_id})
            }
            SessionCommand::CloseFork { .. } if self.fail_close => {
                return Err(Error::new("WORKER_EXITED", "worker exited"));
            }
            SessionCommand::CloseFork { fork_id } => {
                assert_ne!(
                    fork_id, self.active,
                    "the worker refuses closing its active fork"
                );
                self.forks.remove(&fork_id).unwrap();
                json!({"closedFork": fork_id})
            }
            SessionCommand::Accounts { pubkeys } => {
                let current = self.current().clone();
                json!(pubkeys
                    .iter()
                    .map(|key| current
                        .get(key)
                        .map(|l| json!({"pubkey":key,"lamports":l.to_string()})))
                    .collect::<Vec<_>>())
            }
            SessionCommand::Send {
                transaction_base64, ..
            } => {
                let mut output = self.output(&transaction_base64);
                if let Some(rest) = transaction_base64.strip_prefix("credit:") {
                    let (key, lamports) = rest.split_once(':').unwrap();
                    *self.current().entry(key.into()).or_default() +=
                        lamports.parse::<u64>().unwrap();
                }
                if self.diverge {
                    output["computeUnits"] = json!(151);
                }
                output
            }
            SessionCommand::Reset => {
                let base = BTreeMap::from([("a".to_string(), 10)]);
                *self.current() = base;
                json!({"reset": true})
            }
            SessionCommand::SetBalance { pubkey, lamports } => {
                self.current().insert(pubkey, lamports.parse().unwrap());
                json!({"balance": true})
            }
            SessionCommand::OverrideAccounts { entries }
                if entries.iter().all(|e| e.data_base64.is_none()) =>
            {
                for entry in &entries {
                    let lamports: u64 = entry.lamports.as_deref().unwrap_or("0").parse().unwrap();
                    if lamports == 0 {
                        self.current().remove(&entry.pubkey);
                    } else {
                        self.current().insert(entry.pubkey.clone(), lamports);
                    }
                }
                json!({"overrides": entries.len()})
            }
            SessionCommand::OverrideAccounts { entries } => {
                let bytes: usize = entries
                    .iter()
                    .map(|e| e.data_base64.as_ref().map_or(0, String::len))
                    .sum();
                json!({"overrides": entries.len(), "base64Bytes": bytes})
            }
            SessionCommand::RpcSimulate {
                transaction_base64, ..
            } => {
                json!({"signature":"s","output": self.output(&transaction_base64),"admitted":false,"preflightFailed":false})
            }
            other => panic!(
                "unexpected command {}",
                serde_json::to_string(&other).unwrap()
            ),
        })
    }
}

pub(super) fn budget() -> ExecutionBudget {
    ExecutionBudget::new(Duration::from_secs(30), CancellationToken::new()).unwrap()
}

pub(super) fn send(wire: &str) -> SessionCommand {
    SessionCommand::Send {
        transaction_base64: wire.into(),
        sig_verify: false,
    }
}

pub(super) fn lamports(
    forks: &mut Forks,
    branches: &mut Branches,
    branch: u64,
    key: &str,
) -> Value {
    let reply = branches
        .run(
            forks,
            branch,
            SessionCommand::Accounts {
                pubkeys: vec![key.into()],
            },
            &budget(),
        )
        .unwrap();
    reply[0]["lamports"].clone()
}

pub(super) fn simulation(wire: &str, accounts: &[&str]) -> _0_request::Simulation {
    _0_request::Simulation {
        transaction_base64: wire.into(),
        sig_verify: false,
        replace_recent_blockhash: false,
        accounts: accounts.iter().map(|key| key.to_string()).collect(),
    }
}
