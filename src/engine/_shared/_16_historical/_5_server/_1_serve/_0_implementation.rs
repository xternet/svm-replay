use super::*;

/// Serve one verified mutable historical worker over loopback HTTP, and signature and
/// slot subscriptions over WebSocket on `socket` (clients use the HTTP port + 1). The caller
/// reserves both listeners before acquisition/open, and owns the cancellation token.
pub fn serve_session(
    listener: TcpListener,
    socket: TcpListener,
    mut session: VerifiedSession,
    budget: ExecutionBudget,
) -> Result<WorkerOutput, Error> {
    let address = listener
        .local_addr()
        .map_err(|e| Error::new("SESSION_BIND", e.to_string()))?;
    if !address.ip().is_loopback() {
        return Err(Error::new(
            "SESSION_BIND",
            "historical worker RPC must bind loopback",
        ));
    }
    if !socket
        .local_addr()
        .map_err(|e| Error::new("SESSION_BIND", e.to_string()))?
        .ip()
        .is_loopback()
    {
        return Err(Error::new(
            "SESSION_BIND",
            "historical worker WebSocket must bind loopback",
        ));
    }
    listener
        .set_nonblocking(true)
        .map_err(|e| Error::new("SESSION_BIND", e.to_string()))?;
    let mut sockets = _2_socket::Sockets::new(socket)?;
    let mut answered = false;
    loop {
        budget.check().map_err(protocol_error)?;
        session.check()?;
        sockets.poll(&mut session, &budget, std::mem::take(&mut answered))?;
        sockets.sweep();
        let (mut stream, peer) = match listener.accept() {
            Ok(client) => client,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(10));
                continue;
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => {
                return Err(Error::new(
                    "SESSION_HTTP",
                    format!("accept RPC connection: {error}"),
                ))
            }
        };
        let timeout = Some(Duration::from_millis(25));
        stream
            .set_read_timeout(timeout)
            .and_then(|()| stream.set_write_timeout(timeout))
            .map_err(|e| Error::new("SESSION_HTTP", format!("set RPC timeouts: {e}")))?;
        let call = budget
            .narrowed(Duration::from_secs(30))
            .map_err(protocol_error)?;
        let raw = match _0_wire::read(&mut stream, &call) {
            Ok(raw) => raw,
            Err(error) => {
                eprintln!("historical RPC {peer}: {error}");
                let status = match error
                    .details
                    .as_ref()
                    .and_then(|details| details["httpStatus"].as_u64())
                {
                    Some(status) => u16::try_from(status)
                        .map_err(|_| Error::new("SESSION_HTTP", "invalid HTTP failure status"))?,
                    None => 500,
                };
                let body = json!({"jsonrpc":"2.0","id":null,"error":{"code":-32600,"message":error.message}});
                if let Err(error) = _0_wire::write(&mut stream, status, Some(&body), &call) {
                    eprintln!("historical RPC {peer}: failed to send rejection: {error}");
                }
                continue;
            }
        };
        let call = budget
            .narrowed(Duration::from_secs(300))
            .map_err(protocol_error)?;
        let reply = match parse_json(&raw) {
            Ok(value) => session.rpc(&value, &call),
            Err(error) => {
                eprintln!("historical RPC {peer}: malformed JSON: {error}");
                RpcReply {
                    body: Some(json!({"jsonrpc":"2.0","id":null,
                    "error":{"code":-32700,"message":"Parse error"}})),
                    close: false,
                }
            }
        };
        if reply.close {
            sockets.shutdown();
            let output = session.close()?;
            _0_wire::write(
                &mut stream,
                if reply.body.is_some() { 200 } else { 204 },
                reply.body.as_ref(),
                &call,
            )?;
            return Ok(output);
        }
        if let Err(error) = _0_wire::write(
            &mut stream,
            if reply.body.is_some() { 200 } else { 204 },
            reply.body.as_ref(),
            &call,
        ) {
            eprintln!("historical RPC {peer}: response failed: {error}");
        }
        // A send, select or reset may resolve a subscribed signature.
        answered = true;
    }
}
