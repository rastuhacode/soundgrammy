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
        if !self.enabled {
            return;
        }
        self.entries.push(format!(
            "[{}] {}",
            crate::time::now_ms(),
            value.chars().take(8000).collect::<String>()
        ));
        self.trim();
        self.persist();
    }
    pub fn extend(&mut self, values: impl IntoIterator<Item = String>) {
        for value in values {
            self.push(value);
        }
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
            let _ = host.state::<AppState>().db.set_setting(
                "native_diagnostic_logs",
                &serde_json::to_string(&self.entries).unwrap_or_default(),
            );
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
        for n in 0..310 {
            book.push(n.to_string());
        }
        assert_eq!(book.entries.len(), 300);
        assert!(book.entries[0].ends_with(" 10"));
        book.clear();
        assert!(book.entries.is_empty());
    }
}
