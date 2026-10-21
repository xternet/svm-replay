use super::*;

/// The worker capability that takes a proven boundary queue.
pub(in super::super) const AGES: &str = "native-blockhash-ages/v1";
/// Ages a processable recent blockhash can have (MAX_PROCESSING_AGE).
const MAX_AGE: usize = 150;

fn journal(message: &str) -> Error {
    Error::new("JOURNAL_INTEGRITY", message)
}

impl VerifiedSession {
    /// Whether the installed worker declares `capability`.
    pub(in super::super) fn capable(&self, capability: &str) -> bool {
        self.pins["capabilities"]
            .as_array()
            .is_some_and(|caps| caps.iter().any(|cap| cap == capability))
    }

    /// The proof of `blockhash`'s boundary age: pinned parent headers linked back from the
    /// boundary's execution blockhash, read lazily through this session's sources until
    /// they reach it or every processing age. During a journal replay it is the recorded
    /// one. Returns the command and the headers the worker holds once it accepts it.
    pub(in super::super) fn next_queue_proof(
        &mut self,
        blockhash: &str,
    ) -> Result<(SessionCommand, Vec<Value>), Error> {
        if self.replaying {
            let recorded = self
                .side_replay
                .pop_front()
                .ok_or_else(|| journal("the journal records no blockhash proof for this read"))?;
            if recorded["kind"] != "blockhashQueue" || recorded["blockhash"] != blockhash {
                return Err(journal("a recorded side command proves another blockhash"));
            }
            let headers: Vec<Value> = serde_json::from_value(recorded["headers"].clone())
                .map_err(|_| journal("recorded blockhash proof malformed"))?;
            let command = SessionCommand::ProveBlockhashQueue {
                headers: json!(headers),
            };
            return Ok((command, headers));
        }
        let boundary = self.boundary.clone();
        let proven = self.queue_headers.clone();
        let headers = walk(proven, &boundary, blockhash, |slot| {
            self.archive(&json!({"kind":"header","slot":slot}))
        })?;
        if let Some(log) = &mut self.side_log {
            log.push(json!({"kind":"blockhashQueue","blockhash":blockhash,"headers":headers}));
        }
        let command = SessionCommand::ProveBlockhashQueue {
            headers: json!(headers),
        };
        Ok((command, headers))
    }
}

/// Extends the proven `headers` with pinned parent headers read by `fetch`, newest first: the
/// walk continues below the newest proven header, or starts at the `boundary` block's parent
/// (its blockhash is the boundary's execution blockhash). Each header must link to the one
/// above it. It stops at the header whose previous blockhash is `blockhash`, or once every
/// processing age is covered.
pub(in super::super) fn walk(
    mut headers: Vec<Value>,
    boundary: &Value,
    blockhash: &str,
    mut fetch: impl FnMut(u64) -> Result<Value, Error>,
) -> Result<Vec<Value>, Error> {
    if headers.len() >= MAX_AGE {
        return Err(Error::new(
            "SESSION_PROTOCOL",
            "the boundary queue is already proven to every processing age",
        ));
    }
    let start = headers.last().unwrap_or(boundary);
    let (mut slot, mut expected) = (
        integer(&start["parentSlot"])?,
        text(&start["previousBlockhash"])?.to_owned(),
    );
    loop {
        let age = headers.len() + 1;
        let header = fetch(slot).map_err(|error| named(error, slot, age, blockhash))?;
        let integrity = |what: &str| {
            Error::new(
                "SOURCE_INTEGRITY",
                format!("archived block header {slot}: {what}"),
            )
        };
        let (hash, previous, parent) = match (
            header["blockhash"].as_str(),
            header["previousBlockhash"].as_str(),
            header["parentSlot"].as_u64(),
        ) {
            (Some(hash), Some(previous), Some(parent)) if parent < slot => {
                (hash, previous.to_owned(), parent)
            }
            _ => return Err(integrity("malformed or out-of-order header")),
        };
        if hash != expected {
            return Err(integrity(
                "its blockhash does not link to the boundary chain",
            ));
        }
        headers.push(
            json!({"slot":slot,"blockhash":hash,"previousBlockhash":previous,
            "parentSlot":parent}),
        );
        if previous == blockhash || headers.len() == MAX_AGE {
            return Ok(headers);
        }
        (slot, expected) = (parent, previous);
    }
}

/// A header read that failed keeps its typed source code, naming the header it was for.
fn named(mut error: Error, slot: u64, age: usize, blockhash: &str) -> Error {
    error.message = format!(
        "{} (block header {slot}, age {age} of the boundary queue, for recent blockhash {blockhash})",
        error.message
    );
    let context = json!({"blockHeader":slot,"queueAge":age,"recentBlockhash":blockhash});
    error.details = Some(match error.details.take() {
        Some(Value::Object(mut details)) => {
            details.insert("blockhashProof".into(), context);
            Value::Object(details)
        }
        previous => json!({"blockhashProof":context,"details":previous}),
    });
    error
}
