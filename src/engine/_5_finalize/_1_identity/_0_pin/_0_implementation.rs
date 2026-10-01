use super::*;

pub(crate) struct CoordinatorPin {
    pub(in super::super) executable: PathBuf,
    pub(crate) sha256: Digest,
}

impl CoordinatorPin {
    pub(crate) fn current() -> Result<Self, Error> {
        let executable = std::env::current_exe()
            .map_err(|error| Error::new("COORDINATOR_IDENTITY", error.to_string()))?;
        Self::from_path(executable)
    }
    pub(in super::super) fn from_path(executable: PathBuf) -> Result<Self, Error> {
        let sha256 = Digest::new(
            file_sha256(&executable)
                .map_err(|error| Error::new("COORDINATOR_IDENTITY", error.to_string()))?,
        )?;
        Ok(Self { executable, sha256 })
    }
    pub(crate) fn verify(&self) -> Result<(), Error> {
        let actual = file_sha256(&self.executable)
            .map_err(|error| Error::new("COORDINATOR_IDENTITY", error.to_string()))?;
        if actual != self.sha256.as_str() {
            return Err(Error::new(
                "COORDINATOR_IDENTITY",
                "coordinating executable changed during the job",
            )
            .with_details(json!({"expectedSha256":self.sha256,"observedSha256":actual})));
        }
        Ok(())
    }
    pub(crate) fn verify_identity(&self, expected: &Digest) -> Result<(), Error> {
        if expected != &self.sha256 {
            return Err(Error::new(
                "COORDINATOR_IDENTITY",
                "execution identity differs from per-job coordinator pin",
            ));
        }
        self.verify()
    }
    pub(crate) fn check_result(&self, result: Result<Value, Error>) -> Result<Value, Error> {
        if let Err(error) = self.verify() {
            let identity_details = error.details.clone();
            let primary = result.err();
            let proof = primary
                .as_ref()
                .and_then(|error| error.details.as_ref())
                .and_then(|details| details.get("controlEvidence"));
            return Err(crate::_4_simulate::control::attach(
                error.with_details(
                    json!({"identityDetails":identity_details,"primaryError":primary}),
                ),
                proof,
            ));
        }
        result
    }
}

// Low-level executors retain their explicit caller-managed identity contract.
// Public jobs supply a pin and check it immediately before each publication.
pub(crate) fn publish<T>(
    pin: Option<&CoordinatorPin>,
    action: impl FnOnce() -> Result<T, Error>,
) -> Result<T, Error> {
    if let Some(pin) = pin {
        pin.verify()?;
    }
    action()
}
