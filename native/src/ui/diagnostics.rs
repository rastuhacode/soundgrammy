//! Opt-in, bounded diagnostic history; disabled logging never retains errors.
use crate::{runtime::AppHost, state::AppState};
#[derive(Default)]
pub(super) struct LogBook {
    entries: Vec<String>,
    enabled: bool,
    host: Option<AppHost>,
}
impl LogBook {
    pub fn load(host: AppHost, enabled: bool) -> Self {
        let entries = host
            .state::<AppState>()
            .db
            .get_setting("native_diagnostic_logs")
            .ok()
            .flatten()
            .and_then(|v| serde_json::from_str::<Vec<String>>(&v).ok())
            .unwrap_or_default();
        let mut book = Self {
            entries,
            enabled,
            host: Some(host),
        };
        book.trim();
        book
    }
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }
    pub fn push(&mut self, value: String) {
        self.extend(std::iter::once(value));
    }
    pub fn extend(&mut self, values: impl IntoIterator<Item = String>) {
        if !self.enabled {
            return;
        }
        let mut values = values.into_iter().peekable();
        if values.peek().is_none() {
            return;
        }
        let time = crate::time::now_ms();
        self.entries.extend(
            values
                .map(|value| format!("[{time}] {}", value.chars().take(8000).collect::<String>())),
        );
        self.trim();
        self.persist();
    }
    pub fn clear(&mut self) {
        self.entries.clear();
        self.persist();
    }
    pub fn join(&self, separator: &str) -> String {
        self.entries.join(separator)
    }
    pub fn entries(&self) -> &[String] {
        &self.entries
    }
    fn trim(&mut self) {
        if self.entries.len() > 300 {
            self.entries.drain(..self.entries.len() - 300);
        }
    }
    fn persist(&self) {
        if let Some(host) = &self.host {
            let _ = host.persist_setting("native_diagnostic_logs", self.entries.clone());
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use core::prelude::v1::test;
    #[test]
    fn logs_are_opt_in_bounded_and_clearable() {
        let mut book = LogBook::default();
        book.push("private".into());
        assert!(book.join("").is_empty());
        book.set_enabled(true);
        book.extend((0..310).map(|n| n.to_string()));
        assert_eq!(book.entries.len(), 300);
        assert!(book.entries[0].ends_with(" 10"));
        book.clear();
        assert!(book.entries.is_empty());
    }
}
