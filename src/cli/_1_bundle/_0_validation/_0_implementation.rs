use super::*;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in super::super) struct Entry {
    pub(in super::super) path: String,
    pub(in super::super) sha256: Digest,
    pub(in super::super) bytes: u64,
    pub(in super::super) executable: bool,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in super::super) struct Manifest {
    pub(in super::super) schema: String,
    pub(in super::super) version: String,
    pub(in super::super) binary: String,
    pub(in super::super) catalog: String,
    pub(in super::super) files: Vec<Entry>,
}

pub struct Installed {
    pub binary: PathBuf,
    pub catalog: PathBuf,
    pub catalog_sha256: Digest,
    pub binary_sha256: Digest,
}

pub(in super::super) fn error(code: &str, e: impl std::fmt::Display) -> Error {
    Error::new(code, e.to_string())
}

pub(in super::super) fn binary_name(stem: &str) -> String {
    format!("{stem}{}", std::env::consts::EXE_SUFFIX)
}

pub(in super::super) fn is_executable(path: &Path, meta: &fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        let _path = path;
        meta.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        // Windows has no POSIX execute bit. Bundle hashes still authenticate
        // the file; loading the executable is checked by the process launcher.
        meta.is_file()
            && path
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("exe"))
    }
}

pub(in super::super) fn set_mode(path: &Path, mode: u32) -> Result<(), Error> {
    #[cfg(unix)]
    let permissions = fs::Permissions::from_mode(mode);
    #[cfg(not(unix))]
    let permissions = {
        // Preserve inherited Windows ACLs; map only the read-only attribute.
        // This does not claim POSIX owner-only access on Windows.
        let mut permissions = fs::metadata(path)
            .map_err(|e| error("INSTALL_IO", e))?
            .permissions();
        permissions.set_readonly(mode & 0o200 == 0);
        permissions
    };
    fs::set_permissions(path, permissions).map_err(|e| error("INSTALL_IO", e))
}

pub(in super::super) fn relative(path: &str) -> Result<(), Error> {
    if path.is_empty()
        || path.contains('\\')
        || path
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == "..")
        || Path::new(path)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(error(
            "BUNDLE_PATH",
            "bundle paths must be confined relative files",
        ));
    }
    Ok(())
}

pub(in super::super) fn regular(root: &Path, path: &str) -> Result<PathBuf, Error> {
    relative(path)?;
    let mut current = root.to_path_buf();
    for part in Path::new(path).components() {
        current.push(part);
        let meta = fs::symlink_metadata(&current).map_err(|e| error("BUNDLE_IO", e))?;
        if meta.file_type().is_symlink() {
            return Err(error("BUNDLE_PATH", "symlinks are not bundle files"));
        }
    }
    if !current.is_file() {
        return Err(error("BUNDLE_PATH", "bundle entry is not a regular file"));
    }
    Ok(current)
}

pub(in super::super) fn manifest(path: &Path, pin: &Digest) -> Result<(PathBuf, Manifest), Error> {
    let bytes = read_bounded_file(path, 1024 * 1024).map_err(|e| error("BUNDLE_IO", e))?;
    if Digest::of(&bytes) != *pin {
        return Err(error("BUNDLE_INTEGRITY", "bundle manifest pin differs"));
    }
    let m: Manifest =
        serde_json::from_value(parse_json(&bytes)?).map_err(|e| error("BUNDLE_FORMAT", e))?;
    if m.schema != "svm-replay-bundle/v1"
        || m.version != env!("CARGO_PKG_VERSION")
        || m.files.is_empty()
        || m.files.len() > 32
    {
        return Err(error(
            "BUNDLE_FORMAT",
            "unsupported bundle schema/version/file count",
        ));
    }
    let root = path
        .canonicalize()
        .map_err(|e| error("BUNDLE_IO", e))?
        .parent()
        .ok_or_else(|| error("BUNDLE_PATH", "bundle has no parent"))?
        .to_owned();
    let mut names = BTreeSet::new();
    for f in &m.files {
        if !names.insert(&f.path) || f.path == "bundle.json" || f.bytes > 512 * 1024 * 1024 {
            return Err(error(
                "BUNDLE_FORMAT",
                "duplicate/reserved/oversized bundle entry",
            ));
        }
        let file = regular(&root, &f.path)?;
        let meta = fs::metadata(&file).map_err(|e| error("BUNDLE_IO", e))?;
        if meta.len() != f.bytes
            || file_sha256(&file).map_err(|e| error("BUNDLE_IO", e))? != f.sha256.as_str()
            || (f.executable && !is_executable(&file, &meta))
        {
            return Err(error(
                "BUNDLE_INTEGRITY",
                format!("bundle entry differs: {}", f.path),
            ));
        }
    }
    if !names.contains(&m.binary) || !names.contains(&m.catalog) || m.binary == m.catalog {
        return Err(error(
            "BUNDLE_FORMAT",
            "missing/distinct binary and catalog required",
        ));
    }
    Ok((root, m))
}
