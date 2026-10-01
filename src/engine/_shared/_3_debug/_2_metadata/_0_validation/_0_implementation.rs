use super::*;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvocationIdentity {
    pub invocation_index: u16,
    pub parent_index: Option<u16>,
    pub ancestors: Vec<u16>,
    pub program: String,
    pub caller_program: Option<String>,
    pub depth: u16,
}

#[derive(Clone, Debug)]
pub struct InvocationMetadata {
    pub execution_index: u16,
    pub elf_sha256: Digest,
    pub identity: InvocationIdentity,
    pub fields: BTreeMap<String, String>,
}

pub(in super::super::super) fn index(value: &str) -> Result<u16, Error> {
    require(
        value == "0" || (!value.starts_with('0') && value.bytes().all(|b| b.is_ascii_digit())),
        "DEBUG_METADATA",
        "noncanonical runtime index",
    )?;
    let result = value
        .parse::<u16>()
        .map_err(|e| error("DEBUG_METADATA", e.to_string()))?;
    require(
        result < u16::MAX,
        "DEBUG_METADATA",
        "runtime index outside range",
    )?;
    Ok(result)
}

pub(in super::super) fn pubkey(value: &str) -> Result<(), Error> {
    let decoded = bs58::decode(value)
        .into_vec()
        .map_err(|e| error("DEBUG_METADATA", e.to_string()))?;
    require(
        decoded.len() == 32 && bs58::encode(decoded).into_string() == value,
        "DEBUG_METADATA",
        "runtime public key is not canonical 32-byte base58",
    )
}

impl InvocationMetadata {
    pub fn parse(text: &str) -> Result<Self, Error> {
        require(
            text.len() <= 8192,
            "DEBUG_LIMIT",
            "runtime metadata exceeds 8192 bytes",
        )?;
        let mut fields = BTreeMap::new();
        for item in text.trim().split(';') {
            let (key, value) = item
                .split_once('=')
                .ok_or_else(|| error("DEBUG_METADATA", "metadata pair missing separator"))?;
            require(
                !key.is_empty()
                    && !value.contains('=')
                    && fields.insert(key.to_owned(), value.to_owned()).is_none(),
                "DEBUG_METADATA",
                "duplicate or malformed metadata field",
            )?;
        }
        let get = |key: &str| {
            fields
                .get(key)
                .map(String::as_str)
                .ok_or_else(|| error("DEBUG_METADATA", format!("missing runtime {key}")))
        };
        require(
            get("execution_mode")? == "interpreter-debug",
            "DEBUG_METADATA",
            "live VM is not labelled interpreter-debug",
        )?;
        let execution_index = index(get("execution_index")?)?;
        let elf_sha256 = Digest::new(get("elf_sha256")?)?;
        let program = get("program_id")?.to_owned();
        pubkey(&program)?;
        let invocation_index = index(get("invocation_index")?)?;
        let depth = index(get("cpi_level")?)?
            .checked_add(1)
            .ok_or_else(|| error("DEBUG_METADATA", "depth overflow"))?;
        let parent_index = match get("caller_index")? {
            "none" => None,
            value => Some(index(value)?),
        };
        let caller_program = match get("caller")? {
            "none" => None,
            value => {
                pubkey(value)?;
                Some(value.to_owned())
            }
        };
        let ancestors = match get("ancestors")? {
            "none" => Vec::new(),
            chain => chain.split(',').map(index).collect::<Result<Vec<_>, _>>()?,
        };
        require(
            depth <= 64
                && ancestors.len() == usize::from(depth - 1)
                && ancestors.iter().all(|i| *i < invocation_index)
                && ancestors.windows(2).all(|pair| pair[0] < pair[1]),
            "DEBUG_METADATA",
            "inconsistent runtime ancestry",
        )?;
        match parent_index {
            None => require(
                depth == 1 && caller_program.is_none(),
                "DEBUG_METADATA",
                "root has caller/depth mismatch",
            )?,
            Some(parent) => require(
                ancestors.last() == Some(&parent) && caller_program.is_some(),
                "DEBUG_METADATA",
                "caller does not match runtime ancestor chain",
            )?,
        }
        Ok(Self {
            execution_index,
            elf_sha256,
            identity: InvocationIdentity {
                invocation_index,
                parent_index,
                ancestors,
                program,
                caller_program,
                depth,
            },
            fields,
        })
    }
}
