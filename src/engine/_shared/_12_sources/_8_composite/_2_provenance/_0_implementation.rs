use super::*;

fn io(error: impl std::fmt::Display) -> SourceError {
    SourceError::new("SOURCE_IO_ERROR", format!("discovery provenance: {error}"))
}

impl CompositeSource {
    /// Configure before large acquisitions. Retain complete RPC provenance on
    /// disk, with hash-pinned references in the in-memory observation journal.
    pub fn with_discovery_artifacts(mut self, directory: &Path) -> Result<Self> {
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder.create(directory).map_err(io)?;
        self.discovery_artifacts = Some(std::fs::canonicalize(directory).map_err(io)?);
        Ok(self)
    }

    pub(in super::super) fn retain_discovery_provenance(
        &self,
        observation: &mut Value,
    ) -> Result<()> {
        let (Some(root), Some(provenance)) =
            (&self.discovery_artifacts, observation.get("provenance"))
        else {
            return Ok(());
        };
        let bytes = serde_json::to_vec(provenance).map_err(io)?;
        let hash = Digest::of(&bytes);
        let file = format!("{}.json", hash.as_str());
        let path = root.join(&file);
        match std::fs::symlink_metadata(&path) {
            Ok(metadata) => {
                require(
                    metadata.file_type().is_file(),
                    "SOURCE_INTEGRITY",
                    "provenance artifact is not a regular file",
                )?;
                require(
                    Digest::of(std::fs::read(&path).map_err(io)?) == hash,
                    "SOURCE_INTEGRITY",
                    "retained discovery provenance hash differs",
                )?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                let mut temp = tempfile::NamedTempFile::new_in(root).map_err(io)?;
                temp.write_all(&bytes)
                    .and_then(|_| temp.as_file().sync_all())
                    .map_err(io)?;
                temp.persist_noclobber(&path).map_err(io)?;
                #[cfg(unix)]
                std::fs::File::open(root)
                    .and_then(|f| f.sync_all())
                    .map_err(io)?;
            }
            Err(error) => return Err(io(error)),
        }
        observation
            .as_object_mut()
            .ok_or_else(|| io("observation is not an object"))?
            .remove("provenance");
        observation["provenanceArtifact"] = json!({"file":file,"sha256":hash,"bytes":bytes.len()});
        Ok(())
    }
}
