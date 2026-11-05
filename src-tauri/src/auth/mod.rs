//! Piezas genéricas del flujo OAuth con PKCE: generación del verifier y
//! servidor de retorno en `127.0.0.1`. Lo específico de cada proveedor
//! (URL de login, parámetros del callback, intercambio) vive en `provider`.

pub mod callback;
pub mod pkce;

pub use callback::{CallbackInterpreter, CallbackVerdict, LoopbackServer};
pub use pkce::Pkce;
