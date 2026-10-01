use super::*;

pub(super) static HUMAN: AtomicBool = AtomicBool::new(false);

pub fn interactive() -> bool {
    std::io::stdin().is_terminal()
        && std::io::stdout().is_terminal()
        && std::io::stderr().is_terminal()
}

pub fn enabled() -> bool {
    HUMAN.load(Ordering::Relaxed)
}

pub fn configure(human: bool) {
    HUMAN.store(human, Ordering::Relaxed);
}

pub fn clean(text: &str) -> String {
    text.chars().filter(|c| !c.is_control()).take(500).collect()
}

pub fn display(value: &Value) -> String {
    match value.as_str() {
        Some(text) => clean(text),
        None => clean(&value.to_string()),
    }
}

pub fn section(title: &str) {
    if enabled() {
        eprintln!("\n{}", clean(title));
    }
}

pub fn select(prompt: &str, items: &[&str]) -> Result<Option<usize>, Error> {
    Select::with_theme(&style::MenuTheme)
        .with_prompt(prompt)
        .items(items)
        .default(0)
        .interact_opt()
        .map_err(|e| Error::new("TERMINAL_IO", e.to_string()))
}

pub fn stage(text: &str) {
    if enabled() {
        eprintln!("  {}", style::stderr(&clean(text)));
    }
}
