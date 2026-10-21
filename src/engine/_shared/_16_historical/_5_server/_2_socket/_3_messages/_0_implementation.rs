use super::*;

fn send(connection: &mut Connection, value: &Value) {
    let text = value.to_string();
    if connection
        .stream
        .write_all(&_1_frames::encode(1, text.as_bytes()))
        .is_err()
    {
        connection.open = false;
    }
}

/// JSON-RPC over the socket: signature and slot subscriptions and their notifications.
impl Sockets {
    /// Answer one JSON-RPC message; notifications (no id, like web3.js "ping") are ignored.
    pub(in super::super) fn message(&mut self, index: usize, text: &str) -> bool {
        let Ok(request) = serde_json::from_str::<Value>(text) else {
            send(
                &mut self.connections[index],
                &json!({"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"Parse error"}}),
            );
            return false;
        };
        let id = request["id"].clone();
        if id.is_null() {
            return false;
        }
        let params = request["params"].as_array().cloned().unwrap_or_default();
        let subscribe = |sockets: &mut Self, signature: Option<String>| {
            if sockets.pending.len() >= MAX_SUBSCRIPTIONS {
                return Err(json!({"code":-32602,"message":"too many subscriptions"}));
            }
            let subscription = sockets.next;
            sockets.next += 1;
            sockets.pending.push(Pending {
                connection: index,
                subscription,
                signature,
            });
            Ok(subscription)
        };
        let reply = match request["method"].as_str() {
            Some(method @ ("signatureSubscribe" | "slotSubscribe")) => {
                let signature = match params.first().and_then(Value::as_str) {
                    Some(signature) if crate::shared::history::address(signature, 64).is_ok() => {
                        Some(signature.to_owned())
                    }
                    _ if method == "slotSubscribe" => None,
                    _ => {
                        let error =
                            json!({"code":-32602,"message":"Invalid params: signature required"});
                        send(
                            &mut self.connections[index],
                            &json!({"jsonrpc":"2.0","id":id,"error":error}),
                        );
                        return false;
                    }
                };
                match subscribe(self, signature) {
                    Ok(subscription) => {
                        send(
                            &mut self.connections[index],
                            &json!({"jsonrpc":"2.0","id":id,"result":subscription}),
                        );
                        return true;
                    }
                    Err(error) => error,
                }
            }
            Some(method @ ("signatureUnsubscribe" | "slotUnsubscribe")) => {
                let before = self.pending.len();
                let subscription = params.first().and_then(Value::as_u64);
                let slot = method == "slotUnsubscribe";
                self.pending.retain(|p| {
                    p.connection != index
                        || Some(p.subscription) != subscription
                        || p.signature.is_none() != slot
                });
                send(
                    &mut self.connections[index],
                    &json!({"jsonrpc":"2.0","id":id,"result":self.pending.len() != before}),
                );
                return false;
            }
            _ => {
                json!({"code":-32601,"message":"Method not found: this historical fork serves signature and slot subscriptions only"})
            }
        };
        send(
            &mut self.connections[index],
            &json!({"jsonrpc":"2.0","id":id,"error":reply}),
        );
        false
    }

    pub(in super::super) fn deliver(
        &mut self,
        session: &mut VerifiedSession,
        budget: &ExecutionBudget,
    ) -> Result<(), Error> {
        if self.pending.iter().any(|p| p.signature.is_none()) {
            let slot = session.request(SessionCommand::State, budget)?["slot"]
                .as_u64()
                .ok_or_else(|| failure("native slot missing"))?;
            // Only an advance moves forward; selecting another fork just rebases.
            if self.last_slot.is_some_and(|last| slot > last) {
                let connections = &mut self.connections;
                for pending in self.pending.iter().filter(|p| p.signature.is_none()) {
                    let notification = json!({"jsonrpc":"2.0","method":"slotNotification","params":{
                        "result":{"parent":slot - 1,"root":slot,"slot":slot},
                        "subscription":pending.subscription}});
                    send(&mut connections[pending.connection], &notification);
                }
            }
            self.last_slot = Some(slot);
        } else {
            self.last_slot = None;
        }
        let signatures: Vec<String> = self
            .pending
            .iter()
            .filter_map(|p| p.signature.clone())
            .collect();
        if signatures.is_empty() {
            return Ok(());
        }
        let mut statuses = Vec::new();
        for chunk in signatures.chunks(256) {
            let value = session.request(
                SessionCommand::SignatureStatuses {
                    signatures: chunk.to_vec(),
                },
                budget,
            )?;
            statuses.extend(
                value
                    .as_array()
                    .cloned()
                    .ok_or_else(|| failure("native statuses missing"))?,
            );
        }
        let mut index = 0;
        self.pending.retain(|pending| {
            if pending.signature.is_none() {
                return true;
            }
            let status = &statuses[index];
            index += 1;
            if status.is_null() {
                return true;
            }
            let notification = json!({"jsonrpc":"2.0","method":"signatureNotification","params":{
                "result":{"context":{"slot":status["slot"]},"value":{"err":status["err"]}},
                "subscription":pending.subscription}});
            send(&mut self.connections[pending.connection], &notification);
            false
        });
        Ok(())
    }
}
