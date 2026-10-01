use super::super::TraceOptions;
use serde_json::Value;
use svm_replay_protocol::Error;

impl TraceOptions {
    pub(crate) fn check_completeness(&self, exports: &[Value]) -> Result<(), Error> {
        if !self.require_complete {
            return Ok(());
        }
        if exports.is_empty()
            || exports
                .iter()
                .any(|export| export["artifact"]["status"] != "COMPLETE")
        {
            return Err(Error::new(
                "COLLECTION_INCOMPLETE",
                "Full collection could not finish within safety limits. Partial artifacts are retained, not reported as complete. Set explicit --collect limits only if partial output is acceptable.",
            ));
        }
        Ok(())
    }
}
