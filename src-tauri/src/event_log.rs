//! Registro de diagnóstico: en memoria y, opcionalmente, en un archivo local
//! con rotación (`diagnostico.log`, máx. 512 KiB + una copia anterior).
//!
//! Cada mensaje pasa por [`redact`] con los secretos registrados en ese
//! momento, de modo que tokens, códigos y claves no llegan nunca al registro.
//! Nunca se registran cuerpos de respuesta completos ni URLs con credenciales.

use crate::security::{redact, Secret};
use serde::Serialize;
use std::collections::VecDeque;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

const MAX_ENTRIES: usize = 300;
const MAX_SECRETS: usize = 16;
const MAX_FILE_BYTES: u64 = 512 * 1024;

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
    file: Mutex<Option<PathBuf>>,
}

/// Fecha UTC `AAAA-MM-DD HH:MM:SS` sin dependencias externas.
pub fn format_utc(ms: u64) -> String {
    let secs = (ms / 1000) as i64;
    let (days, rem) = (secs.div_euclid(86_400), secs.rem_euclid(86_400));
    // Algoritmo civil_from_days (H. Hinnant).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + if m <= 2 { 1 } else { 0 };
    format!(
        "{y:04}-{m:02}-{d:02} {:02}:{:02}:{:02}",
        rem / 3600,
        (rem % 3600) / 60,
        rem % 60
    )
}

pub fn format_entry(e: &LogEntry) -> String {
    let level = match e.level {
        LogLevel::Info => "INFO",
        LogLevel::Success => "OK",
        LogLevel::Warning => "AVISO",
        LogLevel::Error => "ERROR",
    };
    format!("{} UTC [{level}] {}", format_utc(e.timestamp_ms), e.message)
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

    /// Activa la escritura en `path` (se crea la carpeta si no existe).
    pub fn set_file(&self, path: PathBuf) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        *self.file.lock().unwrap() = Some(path);
    }

    pub fn file_path(&self) -> Option<PathBuf> {
        self.file.lock().unwrap().clone()
    }

    fn write_file(&self, entry: &LogEntry) {
        let Some(path) = self.file.lock().unwrap().clone() else {
            return;
        };
        if std::fs::metadata(&path)
            .map(|m| m.len() > MAX_FILE_BYTES)
            .unwrap_or(false)
        {
            let _ = std::fs::rename(&path, path.with_extension("log.1"));
        }
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
        {
            let _ = writeln!(f, "{}", format_entry(entry));
        }
    }

    /// Últimas `n` entradas como texto (para copiar el diagnóstico).
    pub fn recent_text(&self, n: usize) -> String {
        let list = self.entries.lock().unwrap();
        let skip = list.len().saturating_sub(n);
        list.iter()
            .skip(skip)
            .map(format_entry)
            .collect::<Vec<_>>()
            .join("\n")
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
        self.write_file(&entry);
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
    fn utc_formatting() {
        assert_eq!(format_utc(0), "1970-01-01 00:00:00");
        assert_eq!(format_utc(1_793_847_296_000), "2026-11-05 02:54:56");
    }

    #[test]
    fn writes_redacted_lines_to_file_and_rotates() {
        let dir = std::env::temp_dir().join(format!("toc-log-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let path = dir.join("diagnostico.log");
        let log = EventLog::default();
        log.set_file(path.clone());
        log.protect(&Secret::new("tok_abcdefghijklmnop"));
        log.info("token tok_abcdefghijklmnop y code=XYZ123");
        let text = std::fs::read_to_string(&path).unwrap();
        assert!(
            text.contains("[INFO]")
                && !text.contains("tok_abcdefghijklmnop")
                && !text.contains("XYZ123")
        );
        std::fs::write(&path, vec![b'x'; (MAX_FILE_BYTES + 10) as usize]).unwrap();
        log.info("tras rotar");
        assert!(path.with_extension("log.1").exists());
        assert!(std::fs::read_to_string(&path)
            .unwrap()
            .contains("tras rotar"));
        assert!(log.recent_text(1).contains("tras rotar"));
        let _ = std::fs::remove_dir_all(dir);
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
