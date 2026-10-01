use super::*;

pub struct InstalledCatalog {
    pub manifest: WorkerCatalog,
    pub root: PathBuf,
}

pub struct ResolvedWorker {
    pub descriptor: WorkerDescriptor,
    pub spec: WorkerSpec,
}

pub struct ResolvedCaptureWorker {
    pub worker: ResolvedWorker,
    pub reference_sha256: Digest,
    pub gate_sha256: Digest,
}

pub(in super::super) fn problem(code: &str, message: impl Into<String>) -> Error {
    Error::new(code, message)
}

pub fn load_catalog(path: &Path, expected: &Digest) -> Result<InstalledCatalog, Error> {
    let bytes =
        read_bounded_file(path, 1024 * 1024).map_err(|e| problem("CATALOG_IO", e.to_string()))?;
    if Digest::of(&bytes) != *expected {
        return Err(problem(
            "CATALOG_INTEGRITY",
            "manifest differs from independently supplied pin",
        ));
    }
    let manifest: WorkerCatalog = serde_json::from_value(parse_json(&bytes)?)
        .map_err(|e| problem("CATALOG_FORMAT", e.to_string()))?;
    if manifest.schema != "svm-replay-workers/v1" || manifest.workers.is_empty() {
        return Err(problem("CATALOG_FORMAT", "unknown/empty catalog"));
    }
    check_platform(&manifest)?;
    let canonical = path
        .canonicalize()
        .map_err(|e| problem("CATALOG_IO", e.to_string()))?;
    let root = canonical
        .parent()
        .ok_or_else(|| problem("CATALOG_IO", "manifest has no parent"))?
        .to_path_buf();
    let mut seen = BTreeSet::new();
    for worker in &manifest.workers {
        if !seen.insert(&worker.family)
            || !["v2-2", "v2-3", "v3-0", "v3-1", "v4-0", "v4-1", "v4-2"]
                .contains(&worker.family.as_str())
        {
            return Err(problem("CATALOG_FORMAT", "duplicate or unreviewed family"));
        }
        let capabilities: BTreeSet<_> = worker.capabilities.iter().collect();
        if capabilities.len() != worker.capabilities.len()
            || capabilities.is_empty()
            || worker.executor_source_id.is_empty()
            || worker
                .capabilities
                .iter()
                .any(|v| v.is_empty() || v.len() > 200)
        {
            return Err(problem(
                "CATALOG_FORMAT",
                "invalid capabilities/source identity",
            ));
        }
        let file = confined(&root, &worker.file)?;
        if file_sha256(&file).map_err(|e| problem("CATALOG_IO", e.to_string()))?
            != worker.sha256.as_str()
        {
            return Err(problem("WORKER_IDENTITY", "installed worker bytes differ"));
        }
    }
    let mut capture_families = BTreeSet::new();
    for role in &manifest.capture_workers {
        let worker = &role.worker;
        let reference = manifest
            .workers
            .iter()
            .find(|reference| reference.family == worker.family)
            .ok_or_else(|| problem("CATALOG_FORMAT", "capture role has no baseline family"))?;
        let capabilities: BTreeSet<_> = worker.capabilities.iter().collect();
        if !capture_families.insert(&worker.family)
            || role.reference_sha256 != reference.sha256
            || worker.executor_source_id != reference.executor_source_id
            || capabilities.len() != worker.capabilities.len()
            || worker
                .capabilities
                .iter()
                .any(|value| value.is_empty() || value.len() > 200)
            || !capabilities.contains(&"runtime-call-journal/v1".to_owned())
        {
            return Err(problem(
                "CATALOG_FORMAT",
                "invalid capture role/reference/capabilities",
            ));
        }
        let file = confined(&root, &worker.file)?;
        if file_sha256(&file).map_err(|error| problem("CATALOG_IO", error.to_string()))?
            != worker.sha256.as_str()
        {
            return Err(problem(
                "WORKER_IDENTITY",
                "installed capture worker bytes differ",
            ));
        }
    }
    Ok(InstalledCatalog { manifest, root })
}

pub(in super::super) fn confined(root: &Path, relative: &str) -> Result<PathBuf, Error> {
    if relative.is_empty()
        || relative.contains('\\')
        || Path::new(relative)
            .components()
            .any(|c| !matches!(c, Component::Normal(_)))
    {
        return Err(problem(
            "CATALOG_PATH",
            "worker must be a confined relative path",
        ));
    }
    let file = root
        .join(relative)
        .canonicalize()
        .map_err(|e| problem("CATALOG_IO", e.to_string()))?;
    if !file.starts_with(root) || !file.is_file() {
        return Err(problem(
            "CATALOG_PATH",
            "worker path escapes installation or is not regular",
        ));
    }
    Ok(file)
}

pub(in super::super) fn version(value: &str) -> Result<(u32, u32), Error> {
    let mut parts = value.split('.');
    let major = parts
        .next()
        .ok_or_else(|| problem("CATALOG_FORMAT", "missing libc major"))?
        .parse()
        .map_err(|_| problem("CATALOG_FORMAT", "invalid libc major"))?;
    let minor = parts
        .next()
        .ok_or_else(|| problem("CATALOG_FORMAT", "missing libc minor"))?
        .parse()
        .map_err(|_| problem("CATALOG_FORMAT", "invalid libc minor"))?;
    if parts.next().is_some() {
        return Err(problem(
            "CATALOG_FORMAT",
            "libc version requires major.minor",
        ));
    }
    Ok((major, minor))
}

pub(in super::super) fn check_platform(catalog: &WorkerCatalog) -> Result<(), Error> {
    if catalog.platform.os != std::env::consts::OS
        || catalog.platform.arch != std::env::consts::ARCH
        || !cfg!(all(
            any(target_arch = "x86_64", target_arch = "aarch64"),
            any(
                all(target_os = "linux", target_env = "gnu"),
                target_os = "macos",
                target_os = "windows"
            )
        ))
    {
        return Err(problem(
            "UNSUPPORTED_PLATFORM",
            "worker catalog requires a matching supported native OS and architecture",
        ));
    }
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
    {
        let minimum = catalog
            .platform
            .glibc_min
            .as_deref()
            .ok_or_else(|| problem("CATALOG_FORMAT", "Linux GNU catalog requires glibcMin"))?;
        let required = version(minimum)?;
        // libc provides a static NUL-terminated string for the process lifetime.
        let current = unsafe { std::ffi::CStr::from_ptr(libc::gnu_get_libc_version()) }
            .to_str()
            .map_err(|e| problem("UNSUPPORTED_PLATFORM", e.to_string()))?;
        if version(current)? < required {
            return Err(problem(
                "UNSUPPORTED_PLATFORM",
                format!("glibc {current} is below {minimum}"),
            ));
        }
    }
    #[cfg(not(all(target_os = "linux", target_env = "gnu")))]
    {
        if catalog.platform.glibc_min.is_some() {
            return Err(problem(
                "CATALOG_FORMAT",
                "glibcMin is only valid on Linux GNU",
            ));
        }
    }
    Ok(())
}
