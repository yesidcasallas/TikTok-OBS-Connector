# OAuth y manejo de credenciales — Virelo

## 1. Qué integración se usa hoy

| Función | Servicio | Credencial | Propietario de la integración |
|---|---|---|---|
| Iniciar sesión | Streamlabs (`streamlabs.com/slobs/login` → login de TikTok) | Token de sesión de Streamlabs (`oauth_token`) | **Streamlabs**: la pantalla de autorización de TikTok muestra la app de Streamlabs, no Virelo |
| Estado del acceso a LIVE | `GET streamlabs.com/api/v5/slobs/tiktok/info` | El mismo token, como Bearer | Streamlabs |
| Crear y finalizar el LIVE, obtener RTMP y clave | `POST …/tiktok/stream/start` y `…/stream/{id}/end` | El mismo token | Streamlabs (su acceso de partner con TikTok) |
| Solicitar acceso a LIVE | Página de TikTok `live_access_pc_apply` (id `GL6399433079641606942`) | Sesión de TikTok en el navegador del usuario | Aplicación de acceso de Streamlabs en TikTok |
| StreamElements | No se usa: su id `SE7403650104517334022` solo está centralizado como referencia | — | — |

Virelo **no tiene credenciales propias de TikTok**. Cambiar el nombre de la interfaz no convierte la integración de Streamlabs en propia; por eso la pantalla de login de Virelo dice: «la autorización ante TikTok la concede Streamlabs, no Virelo».

## 2. Flujo actual (Streamlabs): revisión

| Punto | Estado | Detalle |
|---|---|---|
| Autorización | Correcto | Se abre el navegador del sistema. Las credenciales de TikTok solo se escriben en la página de Streamlabs o TikTok. |
| PKCE | Correcto (variante de Streamlabs) | Verifier de 64 bytes aleatorios en hexadecimal (128 caracteres) y challenge `BASE64URL(SHA-256)`, igual que Streamlabs Desktop. Vector del RFC 7636 en las pruebas (`auth/pkce.rs`). |
| `state` | No disponible en el proveedor | Streamlabs no lo envía ni lo devuelve (Desktop tampoco lo usa). Mitigaciones: PKCE (un código ajeno no se puede canjear sin nuestro verifier), puerto aleatorio, validación de `Host`, una sola respuesta y cierre del servidor (`auth/callback.rs`). |
| Callback | Correcto | Solo en `127.0.0.1` y en un puerto libre, con timeout de 5 minutos, cancelación y cierre garantizado (11 pruebas). |
| Intercambio del código | Correcto | `GET /api/v5/slobs/auth/data?code_verifier&code`, sin secreto: es un cliente público de Streamlabs. Los errores no incluyen la URL (`without_url()`). |
| Renovación | No aplica | Virelo no renueva el token de Streamlabs. Si caduca (HTTP 401), se cierra la sesión local y se pide volver a entrar. |
| Cierre de sesión | Solo local | Borra el token de la memoria y del llavero, incluido el servicio antiguo. No se conoce un endpoint de revocación de Streamlabs. |

## 3. Integración propia con TikTok Login Kit (preparada, no activa)

Código: `src-tauri/src/provider/tiktok_login_kit.rs`, con 6 pruebas.

| Requisito de TikTok (documentación vigente) | Implementación |
|---|---|
| `https://www.tiktok.com/v2/auth/authorize/` con `client_key`, `scope`, `redirect_uri`, `state`, `response_type=code`, `code_challenge` y `code_challenge_method=S256` | `authorize_url()`; una prueba comprueba cada parámetro |
| Verifier de 43 a 128 caracteres no reservados, nuevo en cada intento | Se reutiliza `auth::pkce::Pkce` (128 caracteres hexadecimales) |
| «You must use hex encoding of SHA256 to generate the code challenge» | `code_challenge_hex()`; prueba con el vector FIPS de `"abc"`. **Difiere del RFC 7636**, que usa base64url; está documentado en el código |
| Redirect URI de escritorio: `localhost` o `127.0.0.1`, con puerto (se admite `*`), estático y sin parámetros | `REGISTERED_REDIRECT_URI = http://127.0.0.1:*/callback/` y `REDIRECT_PATH = /callback/` |
| Comparar `state` en el callback | `OAuthState` (256 bits) con comparación en tiempo constante. Un `state` ausente o distinto se ignora, sin cancelar el intento |
| Errores del callback (`error`, `error_description`) | Se convierten en `AuthDenied` con el texto saneado |
| `POST https://open.tiktokapis.com/v2/oauth/token/` exige `client_key`, **`client_secret`**, `code`, `grant_type`, `redirect_uri` y `code_verifier` (obligatorio en escritorio) | **No se hace en el cliente.** Lo hará un servidor intermediario de Virelo (ver § 4) |
| Access token de 2 h y refresh token de 180 días, que **rota en cada renovación** | La renovación es tarea del servidor intermediario, que guarda el refresh token |
| «Each request includes your client secret, which must never be exposed to users» | El ejecutable no contiene ningún secreto; hay una prueba que comprueba que la URL de autorización no incluye `client_secret` |

**Activación:** compilar con `VIRELO_TIKTOK_CLIENT_KEY` y `VIRELO_TOKEN_BROKER_URL` (HTTPS). Sin ambas variables, `LoginKitConfig::from_build_env()` devuelve `None` y no hay ningún cambio visible.

## 4. Servidor intermediario (pendiente)

Contrato propuesto, sin implementar:

```text
POST {VIRELO_TOKEN_BROKER_URL}/tiktok/token
  Cuerpo:    { code, code_verifier, redirect_uri }       ← lo envía la app de escritorio
  Servidor:  añade client_key + client_secret y llama a open.tiktokapis.com/v2/oauth/token/
  Respuesta: { session, open_id, scope, expires_in }     ← sesión de Virelo, nunca el client_secret
```

- **Secretos y tokens:** el `client_secret` y el refresh token se guardan solo en el servidor, en un almacén de secretos y no en el repositorio.
- **Sesión:** la app recibe una sesión de Virelo con caducidad y la guarda en el llavero del sistema, como hoy el token de Streamlabs.
- **Cierre de sesión:** debe revocar el token en TikTok desde el servidor (revocación documentada en «Revoke a user access token»).
- **Registros:** igual que en la app, sin tokens ni códigos.

## 5. Tipos de credencial y su finalidad

| Credencial | Finalidad | Dónde vive | Persistencia |
|---|---|---|---|
| Token de sesión de **Streamlabs** (`oauth_token`) | Llamar al API de Streamlabs: estado del acceso, crear y finalizar LIVE | Rust (`Secret`). Nunca llega a la interfaz | Solo con «Recordar»: llavero del sistema, servicio `Virelo` (migrado desde `TikTokOBSConnector`) |
| `access_token` de **TikTok** (Login Kit) | Identificar al usuario (`user.info.basic`). **No** sirve para RTMP | Hoy no existe. En el futuro, en el servidor intermediario | Servidor |
| `refresh_token` de **TikTok** | Renovar el access token (rota) | Solo en el servidor intermediario | Servidor |
| `client_secret` de **TikTok** | Intercambio y renovación de tokens | Solo en el servidor intermediario | Almacén de secretos del servidor |
| **Clave de transmisión** RTMP | Emitir desde OBS | Rust (`Secret`). Solo llega a la interfaz al pulsar «Mostrar»; «Copiar» va del backend al portapapeles | **Nunca** se guarda en disco |
| Código, verifier y `state` | Un intento de login | Rust (`Secret`), borrados al terminar el intento | Nunca |

## 6. Evidencias de que no hay filtraciones

- **Instantáneas y registros:** `assert_no_secrets` (en `app_core/tests.rs`) comprueba que ni el token ni la clave aparecen en las instantáneas de la interfaz, en el registro ni en «Copiar diagnóstico».
- **Redacción:** `security/redact.rs` elimina `code=`, `code_verifier=`, `oauth_token`, `key` y `Bearer …` de cualquier texto registrado.
- **Diagnóstico del proveedor:** `diagnostics_record_status_without_secrets` comprueba que el registro no contiene ni el token ni la clave de una respuesta.
- **Preferencias:** `prefs.rs` tiene una prueba que impide guardar campos como `token`, `key`, `code` o `verifier` en `preferences.json`.
- **Documentación y capturas:** no contienen tokens. Las capturas de la interfaz solo muestran la pista `••••xxxx` de la clave.
