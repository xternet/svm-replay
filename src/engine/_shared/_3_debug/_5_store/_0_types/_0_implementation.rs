use super::*;

pub(in super::super) const MAX_FILE: usize = 256 * 1024 * 1024;

pub(in super::super) const MAX_SYMBOLS: usize = 32 * 1024 * 1024;

pub(in super::super) const MAX_MANIFEST: usize = 1024 * 1024;

pub(in super::super) const MAX_BUNDLE: usize = 1024 * 1024 * 1024;

pub(in super::super) const ASSURANCE: &str =
    "externally-pinned-local-build; integrity-only; not-historical-certification";

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionPin {
    pub manifest_sha256: Digest,
    pub worker_sha256: Digest,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileReference {
    pub sha256: Digest,
    pub bytes: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionLineage {
    pub kind: String,
    pub parent_manifest_sha256: Digest,
    pub parent_session_identity: Digest,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionSymbols {
    pub status: String,
    pub elf_sha256: Option<Digest>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionManifest {
    pub schema: String,
    pub assurance: String,
    pub reference: WorkerDescriptor,
    pub capture: CaptureWorkerDescriptor,
    pub owner_sha256: Digest,
    pub implementation_sha256: Digest,
    pub policy: CaptureRequest,
    pub bounds: ProducerBounds,
    pub capture_identity: Digest,
    pub session_identity: Digest,
    pub symbols: SessionSymbols,
    pub lineage: Option<SessionLineage>,
    pub files: BTreeMap<String, FileReference>,
}

pub struct SaveOptions<'a> {
    pub request: &'a PreparedRequest,
    pub reference: &'a WorkerSpec,
    pub reference_descriptor: &'a WorkerDescriptor,
    pub capture: &'a WorkerSpec,
    pub capture_descriptor: &'a CaptureWorkerDescriptor,
    pub owner: &'a ProcessOwner,
    pub implementation_sha256: &'a Digest,
    pub policy: &'a CaptureRequest,
    pub bounds: &'a ProducerBounds,
    /// Expected digest must originate from immutable replay evidence, not a newly hashed upload.
    pub symbols: Option<(&'a Path, &'a Digest)>,
}

pub struct SavedSession {
    pub directory: PathBuf,
    pub pin: SessionPin,
    pub manifest: SessionManifest,
    pub request: PreparedRequest,
    pub control: PreparedRequest,
    pub reference: WorkerSpec,
    pub capture: WorkerSpec,
    pub owner: ProcessOwner,
}

impl SavedSession {
    /// Source availability is evaluated only on explicit caller request.
    pub fn admit_symbols(&self) -> Result<Option<super::super::super::ExactSymbols>, Error> {
        let verified = open_session(&self.directory, &self.pin)?;
        match verified.manifest.symbols.elf_sha256 {
            Some(pin) => Ok(Some(super::super::super::ExactSymbols::admit(
                &verified.directory.join("symbols.elf"),
                &pin,
            )?)),
            None => Ok(None),
        }
    }
}
