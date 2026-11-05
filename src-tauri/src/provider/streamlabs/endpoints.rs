//! Contrato del API interno de Streamlabs Desktop. Actualice aquí si cambia.

pub const DEFAULT_BASE_URL: &str = "https://streamlabs.com";

/// Página de login. Con estos parámetros Streamlabs redirige a
/// `/tiktok/login` y, al terminar, a `http://127.0.0.1:<port>/?success=true&code=…`
/// (o `success=false&reason=…`).
pub const LOGIN_PATH: &str = "/slobs/login";

/// Parámetros fijos de la URL de login, en el orden que usa Streamlabs
/// Desktop. `None` = parámetro sin valor.
pub const LOGIN_FLAGS: &[(&str, Option<&str>)] = &[
    ("skip_splash", Some("true")),
    ("external", Some("electron")),
    ("tiktok", None),
    ("force_verify", None),
    ("origin", Some("slobs")),
];

/// Intercambio PKCE: `GET ?code_verifier=…&code=…` → `{ success, data: { oauth_token, … } }`.
/// Con código o verifier inválidos responde HTTP 401.
pub const AUTH_DATA_PATH: &str = "/api/v5/slobs/auth/data";

/// Estado del acceso a LIVE (`reason`, `can_be_live`, `user`,
/// `application_status`, `audience_controls_info`). Con `?category=…`
/// devuelve `{ categories: [{ full_name, game_mask_id }] }`.
pub const TIKTOK_INFO_PATH: &str = "/api/v5/slobs/tiktok/info";

/// Crea la sesión LIVE. Multipart: `title`, `device_platform`, `category`,
/// `audience_type`. Respuesta: `{ id, rtmp, key, … }`.
pub const STREAM_START_PATH: &str = "/api/v5/slobs/tiktok/stream/start";

/// Finaliza la sesión LIVE: `POST …/stream/{id}/end` → `{ success }`.
pub const STREAM_END_PREFIX: &str = "/api/v5/slobs/tiktok/stream";
pub const STREAM_END_SUFFIX: &str = "end";

/// Parámetros del callback local.
pub const CALLBACK_SUCCESS: &str = "success";
pub const CALLBACK_CODE: &str = "code";
pub const CALLBACK_REASON: &str = "reason";

/// Según el proyecto de referencia, búsquedas de más de 25 caracteres
/// provocan un HTTP 500.
pub const CATEGORY_QUERY_MAX_CHARS: usize = 25;

/// Nombre que se muestra para la categoría vacía ("Other" en Streamlabs).
pub const OTHER_CATEGORY_NAME: &str = "Otra (sin categoría)";

/// Longitud máxima del título que acepta la aplicación (comprobación local;
/// el límite real lo impone TikTok y no está documentado).
pub const TITLE_MAX_CHARS: usize = 100;

/// Valor real de `device_platform`. Streamlabs Desktop envía
/// `process.platform` de Node (`win32`, `darwin`, `linux`). Aquí se envía el
/// sistema en el que se ejecuta la aplicación: **no** se finge Windows.
pub fn device_platform() -> &'static str {
    match std::env::consts::OS {
        "windows" => "win32",
        "macos" => "darwin",
        other => other,
    }
}

/// `User-Agent` honesto que identifica a esta aplicación.
pub fn user_agent() -> String {
    format!(
        "TikTokOBSConnector/{} ({}; {})",
        env!("CARGO_PKG_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

/// Identificador de la aplicación de acceso a LIVE desde PC de Streamlabs en
/// TikTok (`get id()` de `app/services/platforms/tiktok.ts`).
pub const TIKTOK_APPLICATION_ID: &str = "GL6399433079641606942";

/// Solicitar el acceso a LIVE con Streamlabs (`applicationUrl` en Desktop).
pub const ACCESS_APPLY_URL: &str =
    "https://www.tiktok.com/falcon/live_g/live_access_pc_apply/intro/index.html?id=GL6399433079641606942&lang=es";

/// Consultar el resultado de la solicitud (`confirmationUrl` en Desktop; es
/// también el enlace del proyecto de referencia).
pub const ACCESS_STATUS_URL: &str =
    "https://www.tiktok.com/falcon/live_g/live_access_pc_apply/result/index.html?id=GL6399433079641606942&lang=es";
