use super::*;

pub fn pack(
    catalog: &Path,
    pin: &Digest,
    output: &Path,
    families: &[String],
    reference_only: bool,
) -> Result<Value, Error> {
    let installed = load_catalog(catalog, pin)?;
    let mut catalog: WorkerCatalog = installed.manifest;
    let selected: BTreeSet<_> = families.iter().collect();
    if selected.len() != families.len()
        || selected
            .iter()
            .any(|family| !catalog.workers.iter().any(|w| &w.family == *family))
    {
        return Err(error(
            "UNSUPPORTED_RUNTIME",
            "requested family is duplicate or not installed",
        ));
    }
    if !selected.is_empty() {
        catalog.workers.retain(|w| selected.contains(&w.family));
        catalog
            .capture_workers
            .retain(|w| selected.contains(&w.worker.family));
    }
    if reference_only {
        catalog.capture_workers.clear();
    }
    new_dir(output)?;
    let binary = std::env::current_exe().map_err(|e| error("INSTALL_IO", e))?;
    let binary_pin = Digest::new(file_sha256(&binary).map_err(|e| error("INSTALL_IO", e))?)?;
    let binary_path = format!("bin/{}", binary_name("svm-replay"));
    let mut files = vec![copy(output, &binary, &binary_path, true, &binary_pin)?];
    for (role, worker) in catalog.workers.iter_mut().map(|w| ("reference", w)).chain(
        catalog
            .capture_workers
            .iter_mut()
            .map(|c| ("capture", &mut c.worker)),
    ) {
        let name = format!(
            "workers/{}",
            binary_name(&format!("{}-{role}", worker.family))
        );
        files.push(copy(
            output,
            &installed.root.join(&worker.file),
            &name,
            true,
            &worker.sha256,
        )?);
        worker.file = name;
    }
    let bytes = serde_json::to_vec(&catalog).map_err(|e| error("INSTALL_IO", e))?;
    write_new(&output.join("catalog.json"), &bytes)?;
    files.push(Entry {
        path: "catalog.json".into(),
        sha256: Digest::of(&bytes),
        bytes: bytes.len() as u64,
        executable: false,
    });
    let m = Manifest {
        schema: "svm-replay-bundle/v1".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        binary: binary_path,
        catalog: "catalog.json".into(),
        files,
    };
    let bytes = serde_json::to_vec(&m).map_err(|e| error("INSTALL_IO", e))?;
    let pin = Digest::of(&bytes);
    // Publish manifest last. Partial directories remain visible for diagnosis.
    write_new(&output.join("bundle.json"), &bytes)?;
    open(&output.join("bundle.json"), &pin)?;
    Ok(
        json!({"outcome":"COMPLETED","schema":"svm-replay-installation/v1","bundlePath":output.join("bundle.json"),"bundleSha256":pin,
        "binarySha256":binary_pin,"bytes":m.files.iter().map(|f|f.bytes).sum::<u64>(),"files":m.files.len()}),
    )
}

pub fn install(path: &Path, pin: &Digest, output: &Path) -> Result<Value, Error> {
    open(path, pin)?;
    let (root, m) = manifest(path, pin)?;
    new_dir(output)?;
    for entry in &m.files {
        copy(
            output,
            &regular(&root, &entry.path)?,
            &entry.path,
            entry.executable,
            &entry.sha256,
        )?;
    }
    copy(output, path, "bundle.json", false, pin)?;
    let installed = open(&output.join("bundle.json"), pin)?;
    write_new(
        &output.join("installation.json"),
        &serde_json::to_vec(&json!({"schema":"svm-replay-installation-pin/v1","bundleSha256":pin}))
            .map_err(|e| error("INSTALL_IO", e))?,
    )?;
    Ok(
        json!({"outcome":"COMPLETED","schema":"svm-replay-installation/v1","bundlePath":output.join("bundle.json"),"bundleSha256":pin,"binary":installed.binary,"binarySha256":installed.binary_sha256}),
    )
}
