//! Proveedor Streamlabs (API interno de Streamlabs Desktop para TikTok LIVE).
//!
//! Contrato verificado el 2026-11-04 contra:
//! - el código abierto de Streamlabs Desktop (`app/services/user/auth-module.ts`,
//!   `app/services/platforms/tiktok.ts`, `tiktok/api.ts`),
//! - el proyecto de referencia Loukious/StreamLabsTikTokStreamKeyGenerator
//!   (`TokenRetriever.py`, `Stream.py`),
//! - y peticiones reales sin credenciales válidas (respuestas 302/401).
//!
//! Son endpoints **no documentados**: pueden cambiar o bloquearse sin aviso.

pub mod client;
pub mod endpoints;
pub mod errors;
pub mod models;

pub use client::StreamlabsProvider;
