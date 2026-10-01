use super::*;

pub const ISSUE_URL: &str =
    "https://github.com/xternet/svm-replay/issues/new?template=bug_report.md";

/// Only a known internal invariant failure is automatically reportable. Never
/// copy arbitrary error messages, receipts, environment variables or arguments.
pub fn write_if_internal(path: &Path, result: &Value) -> std::io::Result<bool> {
    if result["outcome"] != "ERROR" || result["error"]["code"] != "INTERNAL" {
        return Ok(false);
    }
    let body = format!(
        "# SVM Replay internal error\n\nVersion: {}\nOS: {}\nArchitecture: {}\nError code: INTERNAL\n\n\
         ## Reproduce\nDescribe what you were doing and add sanitized steps here.\n\n\
         ## Expected / actual result\nDescribe the expected result; the CLI reported an internal error.\n\n\
         Runtime identity is not collected automatically; add reviewed, non-sensitive details if relevant.\n\
         No inputs, raw errors, account data, credentials or local paths were collected.\n\n\
         Review this draft, then paste it into: {ISSUE_URL}\n\
         Do not report security vulnerabilities publicly; follow SECURITY.md.\n",
        env!("CARGO_PKG_VERSION"), std::env::consts::OS, std::env::consts::ARCH
    );
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(body.as_bytes())?;
    Ok(true)
}
