use super::*;

pub(in super::super) const LOADER: &str = "BPFLoaderUpgradeab1e11111111111111111111111";

pub(in super::super) const SYSTEM: &str = "11111111111111111111111111111111";

pub(in super::super) const RENT: &str = "SysvarRent111111111111111111111111111111111";

pub(in super::super) const RELAX: &str = "rexav5eNTUSNT1K2N7cfRjnthwhcP5BC25v2tA4rW4h";

pub(in super::super) const ALPENGLOW: &str = "a1penGLz8Vm2QHYB3JPefBiU4BY3Z6JkW2k3Scw5GWP";

pub(in super::super) struct Migration {
    pub(in super::super) feature: &'static str,
    pub(in super::super) program: &'static str,
    pub(in super::super) data: &'static str,
    pub(in super::super) buffer: &'static str,
    pub(in super::super) executor: &'static str,
}

pub(in super::super) const MIGRATIONS: [Migration; 4] = [
    Migration {
        feature: "ptokFjwyJtrwCa9Kgo9xoDS59V4QccBGEaRFnRPnSdP",
        program: "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA",
        data: "3gvYRKWyXRR9xKWe1ZjPhLY5ZJRN7KDB4rFZFGoJfFk2",
        buffer: "ptok6rngomXrDbWf5v5Mkmu5CEbB51hzSCPDoj9DrvF",
        executor: "litesvm-v0.12.0-agave-3.1.11",
    },
    Migration {
        feature: "Gx4XFcrVMt4HUvPzTpTSVkdDVgcDSjKhDN1RqRS6KDuZ",
        program: "Stake11111111111111111111111111111111111111",
        data: "6WU8Nxarf9fudRK5atWwjLY4vFaw5UrrWhL88qz7iCMJ",
        buffer: "BM11F4hqrpinQs28sEZfzQ2fYddivYs4NEAHF6QMjkJF",
        executor: "litesvm-v0.12.0-agave-3.1.11",
    },
    Migration {
        feature: "STk5Xj8hdAx3sTzmtJ3QysKkq6X2A3yj73JtxttiRyk",
        program: "Stake11111111111111111111111111111111111111",
        data: "6WU8Nxarf9fudRK5atWwjLY4vFaw5UrrWhL88qz7iCMJ",
        buffer: "4EBQBjw1kqF1dqUBb6fc5Ji4tCEQgNf9ESGGX3smwXwh",
        executor: "litesvm-v0.13.1-agave-4.0.0",
    },
    Migration {
        feature: "s51VGwCAgebo2745DSUris72RavoLkXGUmVJosESCXr",
        program: "Stake11111111111111111111111111111111111111",
        data: "6WU8Nxarf9fudRK5atWwjLY4vFaw5UrrWhL88qz7iCMJ",
        buffer: "p51x11QCYMHwuVS1MBcLHKb3MezWyqGS5BEB41CA1dk",
        executor: "litesvm-v0.16.0-agave-4.2.1",
    },
];

impl Migration {
    pub(in super::super) fn keys(&self) -> [&str; 3] {
        [self.program, self.data, self.buffer]
    }
}

pub(in super::super) fn error(error: Error) -> Error {
    Error::new(
        "UNSUPPORTED_BANK_INPUT",
        format!("IMPLEMENTATION_LIMIT:unproven program migration:{error}"),
    )
}

pub(in super::super) fn receipt(source: &Value, method: &str) -> Result<Value> {
    let raw = bytes(field(source, "responseBodyBase64")?)?;
    check(
        field(source, "responseSha256")? == &json!(hash(&raw)),
        "raw receipt integrity",
    )?;
    let request = field(source, "request")?;
    check(
        field(source, "httpStatus")? == &json!(200)
            && field(request, "jsonrpc")? == "2.0"
            && field(request, "method")? == method
            && array(field(request, "params")?)?.len() == 2,
        "request identity",
    )?;
    let envelope = svm_replay_protocol::parse_json(&raw)?;
    let id = |v: &Value| -> Result<String> {
        match v {
            Value::String(s) => Ok(s.clone()),
            Value::Number(_) => Ok(integer(v)?.to_string()),
            _ => Err(fail("RPC id")),
        }
    };
    check(
        field(&envelope, "jsonrpc")? == "2.0"
            && id(field(&envelope, "id")?)? == id(field(request, "id")?)?
            && envelope.get("error").is_none(),
        "response identity/error",
    )?;
    object(field(&envelope, "result")?)?;
    Ok(envelope)
}

pub(in super::super) fn strict(
    account: &Value,
    id: &str,
    slot: u64,
    migration: &Migration,
) -> Result<Value> {
    let mut roles = Roles::default();
    roles.sysvars.insert(RENT.into());
    if field(account, "presence")? != "absent" {
        roles.programs.insert(migration.program.into());
        roles.program_data.insert(migration.data.into());
    }
    let mut value = history::classify_account(account.clone(), id, slot, &roles)?;
    if id == migration.program {
        value["role"] = json!("program");
    }
    if id == migration.data {
        value["role"] = json!("programdata");
    }
    Ok(value)
}

pub(in super::super) fn account(
    source: &Value,
    id: &str,
    slot: u64,
    migration: &Migration,
) -> Result<Value> {
    let envelope = receipt(source, "getAccountInfo")?;
    let result = field(&envelope, "result")?;
    let params = array(field(field(source, "request")?, "params")?)?;
    let options = &params[1];
    check(
        params[0] == id
            && field(options, "slot")? == &json!(slot)
            && field(options, "encoding")? == "base64"
            && field(options, "commitment")? == "finalized"
            && uint(field(field(result, "context")?, "slot")?)? == slot,
        "account boundary",
    )?;
    let data = field(result, "value")?;
    let value = if data.is_null() {
        json!({"pubkey":id,"sourceSlot":slot,"role":"application","presence":"absent"})
    } else {
        let tuple = array(field(data, "data")?)?;
        check(
            tuple.len() == 2 && tuple[1] == "base64",
            "account data encoding",
        )?;
        json!({"pubkey":id,"sourceSlot":slot,"role":"application","presence":"present","owner":field(data,"owner")?,
            "executable":field(data,"executable")?,"lamports":uint(field(data,"lamports")?)?.to_string(),"rentEpoch":uint(field(data,"rentEpoch")?)?.to_string(),"dataBase64":tuple[0]})
    };
    strict(&value, id, slot, migration)
}

pub(in super::super) fn data(account: &Value) -> Result<Vec<u8>> {
    check(
        field(account, "presence")? == "present",
        "account data absent",
    )?;
    bytes(field(account, "dataBase64")?)
}

pub(in super::super) fn get<'a>(map: &'a BTreeMap<String, Value>, id: &str) -> Result<&'a Value> {
    map.get(id)
        .ok_or_else(|| fail(format!("missing migration account:{id}")))
}

pub(in super::super) fn has_feature(features: &[Value], id: &str) -> bool {
    features.iter().any(|v| v.get("id") == Some(&json!(id)))
}
