use super::*;

pub struct RunArtifacts<'a> {
    pub output: &'a Value,
    pub verification: &'a Value,
    pub control_evidence: &'a Value,
    pub events: &'a [Value],
    pub receipt: &'a Value,
    pub exports: &'a [(String, crate::shared::trace::CaptureExport)],
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RunManifest {
    pub schema: String,
    pub status: String,
    pub assurance: String,
    pub session: SessionPin,
    pub session_identity: Digest,
    pub policy: CaptureRequest,
    pub bounds: ProducerBounds,
    pub files: BTreeMap<String, FileReference>,
    pub exports: BTreeMap<String, Digest>,
}

pub struct SavedRun {
    pub directory: PathBuf,
    pub manifest_sha256: Digest,
    pub manifest: RunManifest,
    pub output: Value,
    pub verification: Value,
    pub control_evidence: Value,
    pub events: Vec<Value>,
    pub receipt: Value,
    pub exports: Vec<(String, crate::shared::trace::CaptureExport)>,
}
