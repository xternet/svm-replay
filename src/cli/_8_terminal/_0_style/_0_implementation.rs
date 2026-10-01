use super::*;

pub fn stdout(text: &str) -> String {
    highlight(text, std::io::stdout().is_terminal() && allowed())
}
pub fn path(text: &str) -> String {
    if std::io::stdout().is_terminal() && allowed() {
        format!("\x1b[34m{text}\x1b[0m")
    } else {
        text.into()
    }
}
pub fn stderr(text: &str) -> String {
    highlight(text, std::io::stderr().is_terminal() && allowed())
}
pub(super) fn allowed() -> bool {
    std::env::var_os("NO_COLOR").is_none()
}
pub fn highlight(text: &str, color: bool) -> String {
    if !color {
        return text.into();
    }
    text.split_inclusive(char::is_whitespace)
        .map(|part| {
            let token = part.trim_end();
            let value = token.trim_matches(|c| matches!(c, '(' | ')' | ',' | ';' | '"' | '\''));
            let code = if matches!(value, "✓" | "COMPLETED" | "PASS" | "ok") {
                "32"
            } else if matches!(value, "✗" | "!" | "ERROR" | "FAIL" | "failed" | "error") {
                "31"
            } else if matches!(value, "UNSUPPORTED" | "CANCELLED" | "NEEDS_INPUT") {
                "33"
            } else if matches!(value, "true" | "false" | "null") {
                "35"
            } else if (value.len() == 64 && value.bytes().all(|c| c.is_ascii_hexdigit()))
                || ((32..=88).contains(&value.len())
                    && value.bytes().all(|c| {
                        b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz".contains(&c)
                    }))
            {
                "36"
            } else if value.contains('/') || value.contains('\\') || value.ends_with(".json") {
                "34"
            } else if value.bytes().any(|c| c.is_ascii_digit())
                && value
                    .bytes()
                    .all(|c| c.is_ascii_digit() || b".,+-s".contains(&c))
            {
                "33"
            } else {
                return part.into();
            };
            format!("\x1b[{code}m{token}\x1b[0m{}", &part[token.len()..])
        })
        .collect()
}

pub struct MenuTheme;
impl Theme for MenuTheme {
    fn format_select_prompt_selection(
        &self,
        f: &mut dyn fmt::Write,
        prompt: &str,
        selected: &str,
    ) -> fmt::Result {
        if prompt == "SVM Replay" {
            writeln!(f, "{prompt}: {selected}")
        } else {
            writeln!(f, "{prompt}:\n> {}", stderr(selected))
        }
    }
}
