use super::*;
use std::path::{Component, Path};
pub(in super::super) struct Job {
    pub prepared: Value,
    pub result: Value,
    pub journal: Value,
    pub path: String,
    pub runtime: Value,
}
fn read(path: &Path, pin: &Value, maximum: usize) -> Result<Value, Error> {
    let raw = crate::shared::runtime::read_bounded_file(path, maximum)
        .map_err(|e| Error::new("BOUNDARY_IO", e.to_string()))?;
    if Digest::of(&raw).as_str() != string(pin)? {
        return Err(Error::new(
            "ARTIFACT_INTEGRITY",
            format!("artifact hash differs: {}", path.display()),
        ));
    }
    parse_json(&raw)
}
fn artifact(root: &Path, pin: &Value) -> Result<Value, Error> {
    let file = Path::new(string(field(pin, "file")?)?);
    if file.as_os_str().is_empty() || !file.components().all(|c| matches!(c, Component::Normal(_)))
    {
        return Err(invalid(
            "artifact must be a relative file without traversal",
        ));
    }
    let canonical_root = root
        .canonicalize()
        .map_err(|e| Error::new("BOUNDARY_IO", e.to_string()))?;
    let path = root
        .join(file)
        .canonicalize()
        .map_err(|e| Error::new("BOUNDARY_IO", e.to_string()))?;
    if !path.starts_with(&canonical_root) {
        return Err(invalid("artifact escapes receipt directory"));
    }
    read(&path, field(pin, "sha256")?, 256 * 1024 * 1024)
}
impl Job {
    pub(in super::super) fn load(input: &Value) -> Result<Self, Error> {
        let path = string(field(input, "receiptPath")?)?.to_owned();
        let receipt = read(
            Path::new(&path),
            field(input, "receiptSha256")?,
            16 * 1024 * 1024,
        )?;
        if receipt["outcome"] != "COMPLETED"
            || receipt["controlVerification"]["status"] != "PASS"
            || receipt["verification"]["status"] != "PASS"
        {
            return Err(Error::new(
                "BOUNDARY_UNVERIFIED",
                "job lacks verified original control/result",
            ));
        }
        let root = Path::new(&path)
            .parent()
            .ok_or_else(|| invalid("receipt directory missing"))?;
        let trace = field(input, "trace")?;
        if !array(field(field(&receipt, "trace")?, "exports")?)?
            .iter()
            .any(|e| {
                e["phase"] == "original-control"
                    && e["file"] == trace["file"]
                    && e["sha256"] == trace["sha256"]
            })
        {
            return Err(Error::new(
                "BOUNDARY_UNVERIFIED",
                "journal is not the receipt's pinned original control",
            ));
        }
        let prepared = artifact(root, field(&receipt, "preparedRequest")?)?;
        let result = artifact(root, field(&receipt, "output")?)?;
        let journal = artifact(root, trace)?;
        array(&journal)?;
        let runtime = match receipt.get("runtime") {
            Some(v) => v.clone(),
            None => Value::Null,
        };
        Ok(Self {
            prepared,
            result,
            journal,
            path,
            runtime,
        })
    }
    pub(in super::super) fn fixture(&self) -> Result<&Value, Error> {
        field(&self.prepared, "fixture")
    }
    pub(in super::super) fn target(&self) -> Result<&Value, Error> {
        field(self.fixture()?, "target")
    }
}
