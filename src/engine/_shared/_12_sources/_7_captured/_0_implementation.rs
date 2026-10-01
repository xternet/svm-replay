use super::*;

pub(in super::super) fn io_error(message: &str, path: &Path, error: std::io::Error) -> SourceError {
    let errno = match error.raw_os_error() {
        Some(libc::ENOENT) => "ENOENT".into(),
        Some(libc::EACCES) => "EACCES".into(),
        Some(libc::EPERM) => "EPERM".into(),
        Some(libc::ENOTDIR) => "ENOTDIR".into(),
        Some(libc::EISDIR) => "EISDIR".into(),
        Some(libc::ELOOP) => "ELOOP".into(),
        Some(libc::ENAMETOOLONG) => "ENAMETOOLONG".into(),
        Some(raw) => format!("OS_ERROR_{raw}"),
        None => format!("{:?}", error.kind()),
    };
    SourceError::new("SOURCE_IO_ERROR", format!("{message}: {error}"))
        .details(json!({"path":path,"errno":errno}))
}

pub(in super::super) fn read_bytes(path: &Path) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|e| io_error("captured source file unavailable", path, e))
}

pub(in super::super) struct Entry {
    pub(in super::super) file: String,
    pub(in super::super) digest: Digest,
}

/// Hash-pinned manifest and per-read checked records confined below its directory.
pub struct CapturedSource {
    pub(in super::super) identity: Value,
    pub(in super::super) root: PathBuf,
    pub(in super::super) manifest_hash: Digest,
    pub(in super::super) entries: BTreeMap<Digest, Entry>,
    pub(in super::super) blocks: BTreeMap<u64, Value>,
}

impl CapturedSource {
    pub fn open(manifest_path: &Path, expected_manifest_sha256: &Digest) -> Result<Self> {
        let path = std::fs::canonicalize(manifest_path)
            .map_err(|e| io_error("capture manifest unavailable", manifest_path, e))?;
        let raw = read_bytes(&path)?;
        require(
            Digest::of(&raw) == *expected_manifest_sha256,
            "SOURCE_INTEGRITY",
            "manifest digest mismatch",
        )?;
        let manifest = parse_source_json(&raw)?;
        json_value(&manifest)?;
        object(
            &manifest,
            &["schema", "identity", "entries"],
            "INVALID_SOURCE",
            "manifest",
        )?;
        require(
            manifest["schema"] == "m11-captured-source/v1",
            "INVALID_SOURCE",
            "manifest schema/entries",
        )?;
        let listed = manifest["entries"]
            .as_array()
            .ok_or_else(|| SourceError::new("INVALID_SOURCE", "manifest schema/entries"))?;
        validate_identity(&manifest["identity"])?;
        require(
            manifest["identity"]["kind"] == "captured-history",
            "INVALID_SOURCE",
            "capture kind",
        )?;
        let mut entries = BTreeMap::new();
        let mut blocks = BTreeMap::new();
        for entry in listed {
            object(
                entry,
                &["query", "file", "sha256"],
                "INVALID_SOURCE",
                "manifest entry",
            )?;
            let key = query_key(&entry["query"])?;
            if entry["query"]["kind"] == "block" {
                let at = slot(&entry["query"]["slot"], "INVALID_SOURCE", "block slot")?;
                require(
                    blocks.insert(at, entry["query"].clone()).is_none(),
                    "INVALID_SOURCE",
                    "ambiguous captured block slot",
                )?;
            }
            require(
                covers(&manifest["identity"], &entry["query"])?,
                "INVALID_SOURCE",
                "entry outside declared coverage/capability",
            )?;
            let digest = hash(&entry["sha256"], "INVALID_SOURCE", "record digest")?;
            let file = text(&entry["file"], "INVALID_SOURCE", "record path")?;
            require(
                !file.is_empty()
                    && !Path::new(file).is_absolute()
                    && !file.contains('\\')
                    && file.split('/').all(|part| !matches!(part, "" | "." | "..")),
                "INVALID_SOURCE",
                "record path must stay below manifest directory",
            )?;
            require(
                !entries.contains_key(&key),
                "INVALID_SOURCE",
                "duplicate captured query",
            )?;
            entries.insert(
                key,
                Entry {
                    file: file.into(),
                    digest,
                },
            );
        }
        let root = path
            .parent()
            .ok_or_else(|| SourceError::new("INVALID_SOURCE", "manifest has no parent directory"))?
            .to_path_buf();
        Ok(Self {
            identity: manifest["identity"].clone(),
            root,
            manifest_hash: expected_manifest_sha256.clone(),
            entries,
            blocks,
        })
    }
}

impl HistoricalSource for CapturedSource {
    fn discover_block(&self, slot: u64) -> Result<Option<Value>> {
        match self.blocks.get(&slot) {
            Some(query) => self.inspect(query),
            None => Ok(None),
        }
    }
    fn identity(&self) -> Value {
        self.identity.clone()
    }
    fn diagnostics(&self) -> Result<Value> {
        Ok(local_diagnostics())
    }
    fn inspect(&self, query: &Value) -> Result<Option<Value>> {
        let key = query_key(query)?;
        if !covers(&self.identity, query)? {
            return Ok(None);
        }
        let Some(entry) = self.entries.get(&key) else {
            return Ok(None);
        };
        let named_path = self.root.join(&entry.file);
        let path = std::fs::canonicalize(&named_path)
            .map_err(|e| io_error("captured record unavailable", &named_path, e))?;
        require(
            path.strip_prefix(&self.root)
                .is_ok_and(|relative| !relative.as_os_str().is_empty()),
            "INVALID_SOURCE",
            "record symlink escapes manifest directory",
        )?;
        let raw = read_bytes(&path)?;
        require(
            Digest::of(&raw) == entry.digest,
            "SOURCE_INTEGRITY",
            "record digest mismatch",
        )?;
        let record = parse_source_json(&raw)?;
        validate_record(&record, query)?;
        let mut evidence = evidence_hashes(&record)?;
        evidence.extend([
            self.manifest_hash.as_str().to_owned(),
            entry.digest.as_str().to_owned(),
        ]);
        Ok(Some(
            json!({"query":query,"value":record["value"],"evidenceHashes":unique_hashes(evidence)}),
        ))
    }
}
