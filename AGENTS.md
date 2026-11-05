# AGENTS.md

Guía para agentes de IA (y personas) que trabajan en **Virelo** (antes «TikTok OBS Connector»): app de escritorio para **Windows, Linux y macOS** que inicia sesión con TikTok mediante Streamlabs (OAuth + PKCE en el navegador del sistema), crea la sesión LIVE y entrega el servidor RTMP y la clave para OBS. También finaliza el LIVE.

- **Backend:** Rust + Tauri 2 (`src-tauri/`), `reqwest` con rustls/`ring` y Tokio.
- **Frontend:** React 19 + TypeScript + Vite (`src/`), CSS propio sin frameworks.
- Interfaz **mínima a propósito**: cuenta, título y categoría, servidor y clave, finalizar LIVE. No añada pantallas ni funciones sin que se pidan.
- `README.md` es la documentación de usuario.

## Comandos

```bash
npm install
npm run tauri dev                 # desarrollo (Vite en :1430)
npm run build                     # tsc --noEmit + vite build → dist-web/
npm test                          # Vitest (src/**/*.test.ts)
cd src-tauri && cargo test        # pruebas de Rust; no usan red (proveedor simulado con wiremock)
cd src-tauri && cargo clippy --all-targets
cd src-tauri && cargo test --test streamlabs_contract -- --ignored   # contrato con streamlabs.com (Internet, datos inválidos)

# Instalador
npx tauri build && bash scripts/package-dist.sh                       # en Windows
npx tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc \
  && bash scripts/package-dist.sh x86_64-pc-windows-msvc               # desde WSL/Linux
```

Antes de dar un cambio por terminado: `npm test`, `npm run build`, `cargo test` y `cargo clippy --all-targets` sin avisos. CI (`.github/workflows/build-windows.yml`) repite todo en `windows-latest`.

## Mapa del código

```text
src/
  api.ts                 ÚNICA capa que llama a invoke()/listen().
  types.ts               Espejo de los modelos Rust (camelCase).
  useApp.tsx             Estado global (instantánea `app-state`), avisos, run() sin duplicados.
  components/            AccountCard, StreamForm, CategoryPicker, CredentialsCard, ui.
  lib/text.ts (+ test)   Utilidades puras.
src-tauri/src/
  lib.rs                 Arranque y registro de comandos.
  commands.rs            Comandos Tauri delgados.
  app_core.rs            Orquestación (login, sesión, LIVE, persistencia). Pruebas en app_core/tests.rs.
  auth/pkce.rs           PKCE S256 (verifier de 64 bytes en hex, como Streamlabs Desktop).
  auth/callback.rs       Servidor de retorno en 127.0.0.1 (Host, una sola respuesta, cierre garantizado).
  provider/mod.rs        Trait LiveProvider, independiente del proveedor.
  provider/streamlabs/   endpoints.rs (URLs y parámetros) · client.rs · models.rs · errors.rs
  security/              Secret, redacción de registros, almacén de credenciales (keyring).
  prefs.rs               Preferencias no sensibles (%APPDATA%\com.virelo.desktop), con migración desde los identificadores antiguos.
  provider/tiktok_login_kit.rs  Integración propia con TikTok Login Kit: preparada y NO activa (sin credenciales de Virelo).
docs/integracion-tiktok/  Matriz de requisitos de TikTok, OAuth, acceso LIVE, materiales de revisión y borradores legales.
site/                    Borrador del sitio público (sin publicar).
```

## Reglas obligatorias

### Secretos (token OAuth, código, verifier, clave de transmisión)

- Siempre dentro de `security::Secret`, que a propósito no implementa `Serialize`. No cree DTO que los transporten.
- El token **nunca** sale de Rust. La clave solo sale con `stream_reveal_key`, cuando el usuario pulsa «Mostrar». «Copiar» escribe en el portapapeles desde Rust.
- `Secret::expose()` solo se usa para entregarlo al proveedor, al almacén o al portapapeles; nunca en `format!`, registros ni errores.
- Los errores de `reqwest` se convierten con `without_url()`: la URL del intercambio lleva el código y el verifier.
- Persistencia solo con «Recordar», en el Administrador de credenciales de Windows. Nunca en `preferences.json` ni en `localStorage`.

### Proveedor (Streamlabs)

- Son **endpoints internos no documentados**. Todo lo específico va en `provider/streamlabs/`. Si cambian, se actualizan `endpoints.rs`, `models.rs` y `errors.rs`, y el contrato de `tests/streamlabs_contract.rs`.
- **No suplante plataformas ni clientes:** `device_platform` es el sistema real (`win32` en Windows) y el User-Agent identifica a esta app.
- Distinga siempre autenticación correcta, token inválido (401), permisos insuficientes y fallo del proveedor. Iniciar sesión no implica acceso a LIVE.
- **Un código HTTP o un mensaje genérico no prueban la causa:** `PermissionDenied` solo se usa con códigos explícitos del proveedor. Un 403 sin código es `ProviderForbidden`, y el mensaje original del proveedor se conserva siempre en `detail`.
- El permiso de emisión se modela con `Field<T>` (presente, ausente o inválido) y `Availability` (`provider/mod.rs`, tabla en `models.rs::availability_of`). Un campo ausente o inválido da «desconocido», nunca «no aprobada».
- `start_stream` exige `Availability::Confirmed`, igual que el botón (`ensure_live_permission`). Nunca reintente automáticamente la creación de un LIVE, y no cree sesiones LIVE al comprobar permisos ni en las pruebas.
- Las comprobaciones (`refresh_account`) no retienen el candado; `session_gen` y `check_seq` descartan respuestas tardías o de sesiones anteriores.
- Diagnóstico: `EventLog` (memoria y `diagnostico.log`, con rotación). Registre solo operación, código HTTP, tipo de contenido, duración, campos de estado y errores saneados.
- Las credenciales de TikTok solo se escriben en la página del proveedor. Prohibido extraer cookies o tokens de otras aplicaciones (por ejemplo, del almacenamiento de Streamlabs Desktop).

### Seguridad de Tauri y concurrencia

- Permisos mínimos en `src-tauri/capabilities/default.json` (solo `core:event:allow-listen/unlisten`). Sin plugins de shell, fs ni http en el frontend. Mantenga la CSP y `freezePrototype`.
- El navegador solo se abre desde Rust (`opener.rs`, solo `https://`).
- Las operaciones con el proveedor usan el candado `op.try_lock()` → `AppError::Busy`. Las llamadas bloqueantes (keyring, navegador) van en `spawn_blocking`.

### Marca e integración con TikTok

- El nombre visible es **Virelo**; no use «TikTok» en el nombre de la app (la revisión de TikTok lo rechaza). TikTok, OBS y Streamlabs solo se nombran cuando identifican el servicio real.
- No cambie `identifier`, `KEYRING_SERVICE` ni las rutas de datos sin migración (ver `LEGACY_IDENTIFIERS` en `lib.rs` y `LEGACY_KEYRING_SERVICES`).
- La autorización actual es de **Streamlabs**: no la presente como propia de Virelo. Login Kit no permite crear emisiones ni obtener claves RTMP.
- Nunca incluya un `client_secret` en el ejecutable: el intercambio del código de Login Kit debe hacerlo un servidor de Virelo.

## Convenciones

- Interfaz, errores, registros, comentarios y documentación en **español**; identificadores en inglés.
- `AppError`: mensaje en español orientado a la acción, `code()` estable en SCREAMING_SNAKE_CASE y `category()` (`auth`, `permission`, `provider`, `network` o `app`).
- Modelos con `#[serde(rename_all = "camelCase")]` y espejo exacto en `src/types.ts`.
- TypeScript estricto (`noUnusedLocals`, `noUnusedParameters`).

## Receta: añadir un comando Tauri

1. Lógica en `app_core.rs` o en el proveedor, devolviendo `AppResult<T>`.
2. Comando delgado en `commands.rs`, registrado en `generate_handler!` de `lib.rs`.
3. Tipos en `src/types.ts` y función en `src/api.ts`.
4. Pruebas en `app_core/tests.rs` con `MockServer` (wiremock) y el navegador simulado `FakeBrowser`.

## Publicación

- `.github/workflows/release.yml` publica en GitHub Releases al subir una etiqueta `v*`. El orden es `verify` → `create-release` (borrador) → `build` (matriz de 4 plataformas con `tauri-action`) → `publish` (`SHA256SUMS.txt` y fin del borrador). Con sufijo (`-beta.N`) sale como Pre-release.
- Las acciones están fijadas por SHA y Rust en `RUST_TOOLCHAIN`. Al actualizarlas, resuelva el SHA de la etiqueta y mantenga el comentario con la versión.
- La versión debe coincidir en `package.json`, `package-lock.json`, `Cargo.toml`, `Cargo.lock` y `tauri.conf.json`; `verify` lo comprueba.
- Las notas van en `.github/releases/<etiqueta>.md` (primera línea = título). No afirme funciones ni plataformas sin verificarlas.
- Nunca reutilice ni mueva una etiqueta publicada: si algo falla, publique la siguiente beta.

## Git

- `main` es la rama estable; `develop`, la de integración. Trabaje en ramas desde `develop` y vuelva con pull request.
- No versione binarios: `dist/` está en `.gitignore`. El instalador se genera con CI o con `scripts/package-dist.sh`.
- No añada líneas `Co-Authored-By` ni atribuciones a herramientas en los commits ni en los PR: el propietario debe ser el único contribuidor.
