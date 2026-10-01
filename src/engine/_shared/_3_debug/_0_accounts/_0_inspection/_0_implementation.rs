use super::*;

pub(in super::super) fn address(value: &Value) -> Result<u64, Error> {
    let text = value
        .as_str()
        .ok_or_else(|| error("DEBUG_ACCOUNT", "account address is not a string"))?;
    require(
        text.starts_with("0x")
            && (3..=18).contains(&text.len())
            && text[2..]
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "DEBUG_ACCOUNT",
        "invalid runtime address",
    )?;
    u64::from_str_radix(&text[2..], 16).map_err(|e| error("DEBUG_ACCOUNT", e.to_string()))
}

pub fn account_directory(client: &mut DebugClient) -> Result<Value, Error> {
    directory(client, None)
}

pub(in super::super) fn directory(
    client: &mut DebugClient,
    expected: Option<&InvocationMetadata>,
) -> Result<Value, Error> {
    require(
        client.state() == DebugState::Stopped,
        "DEBUG_STATE",
        "account directory requires paused VM",
    )?;
    let metadata = InvocationMetadata::parse(&client.metadata()?)?;
    if let Some(expected) = expected {
        require(
            metadata.fields == expected.fields,
            "DEBUG_STALE_STATE",
            "paused account metadata no longer matches attached invocation",
        )?;
    }
    require(
        metadata.fields.get("account_layout").map(String::as_str)
            == Some("serialized-addresses-v1"),
        "CAPABILITY_UNAVAILABLE",
        "runtime account metadata lacks serialized-addresses-v1",
    )?;
    let count = metadata
        .fields
        .get("account_count")
        .ok_or_else(|| error("DEBUG_ACCOUNT", "account count missing"))?;
    let count = super::super::super::metadata::index(count)?;
    require(
        count <= 256,
        "DEBUG_ACCOUNT",
        "account count exceeds runtime bound",
    )?;
    let raw = metadata
        .fields
        .get("accounts")
        .ok_or_else(|| error("DEBUG_ACCOUNT", "runtime account directory missing"))?;
    let rows = parse_json(raw.as_bytes())?;
    let rows = rows
        .as_array()
        .ok_or_else(|| error("DEBUG_ACCOUNT", "runtime account directory is not an array"))?;
    require(
        rows.len() <= 16 && rows.len() <= usize::from(count),
        "DEBUG_ACCOUNT",
        "runtime account directory exceeds bound",
    )?;
    let mut accounts = Vec::new();
    for (index, value) in rows.iter().enumerate() {
        let row = value.as_array().ok_or_else(|| {
            error(
                "DEBUG_ACCOUNT",
                "runtime account descriptor is not an array",
            )
        })?;
        require(
            row.len() == 9 && row[0].as_u64() == Some(index as u64),
            "DEBUG_ACCOUNT",
            "account descriptor/index differs",
        )?;
        let pubkey = row[1]
            .as_str()
            .ok_or_else(|| error("DEBUG_ACCOUNT", "runtime account name is missing"))?;
        let decoded = bs58::decode(pubkey)
            .into_vec()
            .map_err(|e| error("DEBUG_ACCOUNT", e.to_string()))?;
        require(
            decoded.len() == 32 && bs58::encode(decoded).into_string() == pubkey,
            "DEBUG_ACCOUNT",
            "runtime account name is not canonical",
        )?;
        let original = row[7]
            .as_u64()
            .ok_or_else(|| error("DEBUG_ACCOUNT", "original account length is not u64"))?;
        let maximum = row[8]
            .as_u64()
            .ok_or_else(|| error("DEBUG_ACCOUNT", "maximum account length is not u64"))?;
        require(
            original <= maximum && maximum <= 10_496_000,
            "DEBUG_ACCOUNT",
            "invalid serialized account length bound",
        )?;
        accounts.push(json!({"instructionIndex":index,"pubkey":pubkey,"key":address(&row[2])?,"owner":address(&row[3])?,"lamports":address(&row[4])?,"length":address(&row[5])?,"data":address(&row[6])?,"originalLength":original,"maximumLength":maximum}));
    }
    Ok(
        json!({"identity":metadata.identity,"executionIndex":metadata.execution_index,"elfSha256":metadata.elf_sha256,"accounts":accounts,"totalAccounts":count,"disposition":if rows.len()==usize::from(count){"COMPLETE"}else{"TRUNCATED"}}),
    )
}

pub fn inspect_account(
    client: &mut DebugClient,
    name: &str,
    max_data_bytes: usize,
) -> Result<Value, Error> {
    require(
        max_data_bytes <= 4096,
        "DEBUG_ACCOUNT",
        "account data bound must be 0..4096",
    )?;
    let directory = account_directory(client)?;
    inspect_directory(client, name, max_data_bytes, &directory)
}

pub(in super::super::super) fn bound_accounts(
    client: &mut DebugClient,
    expected: &InvocationMetadata,
    inspect: Option<(&str, usize)>,
) -> Result<Value, Error> {
    if let Some((_, maximum)) = inspect {
        require(
            maximum <= 4096,
            "DEBUG_ACCOUNT",
            "account data bound must be 0..4096",
        )?;
    }
    let directory = directory(client, Some(expected))?;
    match inspect {
        Some((name, maximum)) => inspect_directory(client, name, maximum, &directory),
        None => Ok(directory),
    }
}
