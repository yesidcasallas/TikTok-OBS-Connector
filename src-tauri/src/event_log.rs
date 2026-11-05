//! Registro de actividad en memoria, visible en la interfaz.
//!
//! Cada mensaje pasa por [`redact`] con los secretos registrados en ese
//! momento, de modo que tokens, códigos y claves no llegan nunca al registro.

use crate::security::{redact, Secret};
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

const MAX_ENTRIES: usize = 300;
const MAX_SECRETS: usize = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LogLevel {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub id: u64,
    pub timestamp_ms: u64,
    pub level: LogLevel,
    pub message: String,
}

pub trait LogSink: Send + Sync {
    fn on_entry(&self, entry: &LogEntry);
}

#[derive(Default)]
pub struct EventLog {
    entries: Mutex<VecDeque<LogEntry>>,
    next_id: AtomicU64,
    sink: Mutex<Option<Arc<dyn LogSink>>>,
    secrets: Mutex<Vec<Secret>>,
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

impl EventLog {
    pub fn set_sink(&self, sink: Arc<dyn LogSink>) {
        *self.sink.lock().unwrap() = Some(sink);
    }

    /// Registra un secreto para eliminarlo de cualquier mensaje futuro.
    pub fn protect(&self, secret: &Secret) {
        if secret.expose().len() < 4 {
            return;
        }
        let mut list = self.secrets.lock().unwrap();
        if !list.iter().any(|s| s == secret) {
            if list.len() >= MAX_SECRETS {
                list.remove(0);
            }
            list.push(secret.clone());
        }
    }

    /// Aplica la redacción con los secretos registrados.
    pub fn sanitize(&self, text: &str) -> String {
        let list = self.secrets.lock().unwrap();
        let refs: Vec<&str> = list.iter().map(|s| s.expose()).collect();
        redact(text, &refs)
    }

    pub fn push(&self, level: LogLevel, message: impl AsRef<str>) {
        let entry = LogEntry {
            id: self.next_id.fetch_add(1, Ordering::Relaxed) + 1,
            timestamp_ms: now_ms(),
            level,
            message: self.sanitize(message.as_ref()),
        };
        #[cfg(debug_assertions)]
        eprintln!("[{:?}] {}", entry.level, entry.message);
        {
            let mut list = self.entries.lock().unwrap();
            if list.len() >= MAX_ENTRIES {
                list.pop_front();
            }
            list.push_back(entry.clone());
        }
        let sink = self.sink.lock().unwrap().clone();
        if let Some(sink) = sink {
            sink.on_entry(&entry);
        }
    }

    pub fn info(&self, m: impl AsRef<str>) {
        self.push(LogLevel::Info, m)
    }
    pub fn success(&self, m: impl AsRef<str>) {
        self.push(LogLevel::Success, m)
    }
    pub fn warn(&self, m: impl AsRef<str>) {
        self.push(LogLevel::Warning, m)
    }
    pub fn error(&self, m: impl AsRef<str>) {
        self.push(LogLevel::Error, m)
    }

    pub fn entries(&self) -> Vec<LogEntry> {
        self.entries.lock().unwrap().iter().cloned().collect()
    }

    pub fn clear(&self) {
        self.entries.lock().unwrap().clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protected_secrets_never_reach_entries() {
        let log = EventLog::default();
        let token = Secret::new("tok_abcdefghijklmnop");
        log.protect(&token);
        log.info("El token es tok_abcdefghijklmnop y Bearer zzz111");
        let e = &log.entries()[0];
        assert!(!e.message.contains("tok_abcdefghijklmnop"));
        assert!(!e.message.contains("zzz111"));
    }

    #[test]
    fn keeps_a_bounded_history() {
        let log = EventLog::default();
        for i in 0..(MAX_ENTRIES + 10) {
            log.info(format!("m{i}"));
        }
        assert_eq!(log.entries().len(), MAX_ENTRIES);
    }
}
