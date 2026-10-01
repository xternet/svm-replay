use super::*;

pub const CLOCK: &str = "SysvarC1ock11111111111111111111111111111111";

pub const INSTRUCTIONS: &str = "Sysvar1nstructions1111111111111111111111111";

pub const RECENT_BLOCKHASHES: &str = "SysvarRecentB1ockHashes11111111111111111111";

pub const UPGRADEABLE_LOADER: &str = "BPFLoaderUpgradeab1e11111111111111111111111";

#[derive(Default, Clone)]
pub struct Roles {
    pub programs: BTreeSet<String>,
    pub program_data: BTreeSet<String>,
    pub address_tables: BTreeSet<String>,
    pub sysvars: BTreeSet<String>,
    pub excluded: BTreeSet<String>,
}

impl Roles {
    pub fn role(&self, key: &str) -> Result<&'static str, Error> {
        if self.excluded.contains(key) {
            return Err(Error::new("UNSUPPORTED_EXCLUDED_ACCOUNT", key));
        }
        Ok(if self.program_data.contains(key) {
            "programdata"
        } else if self.address_tables.contains(key) {
            "address-lookup-table"
        } else if self.sysvars.contains(key) {
            "sysvar"
        } else if self.programs.contains(key) {
            "program"
        } else {
            "application"
        })
    }
}

pub(in super::super) fn invalid(message: impl Into<String>) -> Error {
    Error::new("HISTORICAL_ACCOUNT", message)
}

pub fn address(value: &str, length: usize) -> Result<(), Error> {
    if value.len() > 88 {
        return Err(invalid("address too long"));
    }
    let bytes = bs58::decode(value)
        .into_vec()
        .map_err(|e| invalid(e.to_string()))?;
    if bytes.len() != length || bs58::encode(&bytes).into_string() != value {
        return Err(invalid("noncanonical address"));
    }
    Ok(())
}

pub fn data(value: &str) -> Result<Vec<u8>, Error> {
    let bytes = STANDARD.decode(value).map_err(|e| invalid(e.to_string()))?;
    if STANDARD.encode(&bytes) != value {
        return Err(invalid("noncanonical base64"));
    }
    Ok(bytes)
}

pub fn account_data(account: &Value) -> Result<Vec<u8>, Error> {
    if account["presence"] != "present" {
        return Err(invalid("absent account has no data"));
    }
    data(
        account["dataBase64"]
            .as_str()
            .ok_or_else(|| invalid("account data missing"))?,
    )
}

pub fn classify_account(
    mut value: Value,
    key: &str,
    slot: u64,
    roles: &Roles,
) -> Result<Value, Error> {
    address(key, 32)?;
    if slot > 9_007_199_254_740_991
        || value["pubkey"] != key
        || value["sourceSlot"].as_u64() != Some(slot)
    {
        return Err(Error::new(
            "SOURCE_CONTEXT_MISMATCH",
            format!("{key}: wrong as-of slot/address"),
        ));
    }
    let role = roles.role(key)?;
    let names = match value["presence"].as_str() {
        Some("absent") => {
            if role != "application" {
                return Err(Error::new(
                    "UNSUPPORTED_REQUIRED_ACCOUNT",
                    format!("{key}: required {role} absent at {slot}"),
                )
                .with_details(json!({"pubkey":key,"slot":slot,"role":role})));
            }
            vec!["pubkey", "sourceSlot", "role", "presence"]
        }
        Some("present") => {
            for field in ["lamports", "rentEpoch"] {
                let n = value[field]
                    .as_str()
                    .ok_or_else(|| invalid(format!("{key}: decimal {field} required")))?;
                let parsed = n
                    .parse::<u64>()
                    .map_err(|_| invalid(format!("{key}: invalid u64 {field}")))?;
                if parsed.to_string() != n {
                    return Err(invalid(format!("{key}: noncanonical {field}")));
                }
            }
            address(
                value["owner"]
                    .as_str()
                    .ok_or_else(|| invalid("missing owner"))?,
                32,
            )?;
            if !value["executable"].is_boolean() {
                return Err(invalid("executable must be boolean"));
            }
            account_data(&value)?;
            vec![
                "pubkey",
                "sourceSlot",
                "role",
                "presence",
                "lamports",
                "rentEpoch",
                "owner",
                "executable",
                "dataBase64",
            ]
        }
        _ => return Err(invalid("presence must be explicit present/absent")),
    };
    let object = value
        .as_object_mut()
        .ok_or_else(|| invalid("account must be object"))?;
    if object.len() != names.len() || names.iter().any(|k| !object.contains_key(*k)) {
        return Err(invalid("account fields differ"));
    }
    object.insert("role".into(), json!(role));
    Ok(value)
}

pub fn program_data_address(account: &Value) -> Result<Option<String>, Error> {
    if account["presence"] == "absent"
        || account["executable"] != true
        || account["owner"] != UPGRADEABLE_LOADER
    {
        return Ok(None);
    }
    let bytes = account_data(account)?;
    if bytes.len() != 36 || bytes[..4] != 2u32.to_le_bytes() {
        return Err(invalid("executable upgradeable ProgramData link malformed"));
    }
    Ok(Some(bs58::encode(&bytes[4..36]).into_string()))
}

pub struct History<'a> {
    pub sources: &'a mut CompositeSource,
    pub genesis: String,
    pub target_slot: u64,
    pub budget: &'a ExecutionBudget,
    pub max_reads: usize,
    pub(in super::super) reads: usize,
}
