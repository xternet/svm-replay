use super::*;

pub fn open(path: &Path, pin: &Digest) -> Result<Installed, Error> {
    let (root, m) = manifest(path, pin)?;
    let find = |name: &str| {
        m.files
            .iter()
            .find(|f| f.path == name)
            .ok_or_else(|| error("BUNDLE_FORMAT", "entry missing"))
    };
    let binary = find(&m.binary)?;
    if !binary.executable {
        return Err(error("BUNDLE_FORMAT", "CLI must be executable"));
    }
    let catalog = find(&m.catalog)?;
    let installed = load_catalog(&root.join(&m.catalog), &catalog.sha256)?;
    for worker in installed
        .manifest
        .workers
        .iter()
        .chain(installed.manifest.capture_workers.iter().map(|c| &c.worker))
    {
        // Catalog sits at the bundle root, so both manifests name the same files.
        if m.catalog != "catalog.json" {
            return Err(error("BUNDLE_FORMAT", "catalog must be at bundle root"));
        }
        let entry = find(&worker.file)?;
        if !entry.executable || entry.sha256 != worker.sha256 {
            return Err(error("BUNDLE_INTEGRITY", "worker/catalog entry differs"));
        }
    }
    Ok(Installed {
        binary: root.join(&m.binary),
        binary_sha256: binary.sha256.clone(),
        catalog: root.join(&m.catalog),
        catalog_sha256: catalog.sha256.clone(),
    })
}

/// Return demo inputs only if both files are covered by the verified bundle pin.
pub fn demo_files(path: &Path, pin: &Digest) -> Result<(PathBuf, PathBuf), Error> {
    let (root, manifest) = manifest(path, pin)?;
    for name in ["demo/manifest.json", "demo/request.json"] {
        if !manifest
            .files
            .iter()
            .any(|entry| entry.path == name && !entry.executable)
        {
            return Err(error(
                "DEMO_NOT_INSTALLED",
                "this bundle does not include an approved offline demo",
            ));
        }
    }
    Ok((
        root.join("demo/manifest.json"),
        root.join("demo/request.json"),
    ))
}
