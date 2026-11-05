# TikTok OBS Connector

**Obtén el servidor RTMP y la clave de transmisión de TikTok LIVE para OBS iniciando sesión con tu cuenta de TikTok, sin copiar nada a mano desde LIVE Center.**

TikTok OBS Connector es una aplicación de escritorio para **Windows 10/11**. Inicia sesión con TikTok **a través de Streamlabs** en tu navegador, crea la sesión LIVE con el título y la categoría que elijas y te entrega el **servidor** y la **clave** listos para pegar en OBS. Al terminar, finaliza el LIVE desde la propia aplicación.

Hecha con **Rust + Tauri 2** y **React + TypeScript**. Interfaz mínima, en español, con un instalador de unos pocos MB.

> ⚠️ **Aviso.** Usa **endpoints internos y no documentados de Streamlabs Desktop**. Pueden cambiar o dejar de funcionar sin aviso, y su uso por terceros puede no estar permitido por las condiciones de Streamlabs. No está afiliada a TikTok ni a Streamlabs. **Iniciar sesión no garantiza poder emitir:** TikTok decide qué cuentas tienen acceso a LIVE.

## Funciones

- Iniciar sesión con TikTok mediante Streamlabs en el navegador del sistema (OAuth con PKCE). Tus credenciales solo se escriben en la página de Streamlabs/TikTok.
- Ver la cuenta vinculada y, por separado, el **permiso de emisión**: confirmado, sin acceso o sin confirmar. Incluye «Actualizar estado».
- **Consultar estado** y **Solicitar acceso** en las páginas oficiales de TikTok para Streamlabs cuando la cuenta no tiene permiso. Streamlabs no muestra las solicitudes en revisión (sigue respondiendo `never_applied`), así que el estado real se consulta en TikTok. El formulario del LIVE solo aparece con el permiso confirmado.
- Elegir el título y la categoría (búsqueda de categorías de TikTok).
- **Obtener servidor y clave**: solo se habilita con permiso confirmado, y Rust aplica la misma regla. ⚠️ Esto crea una sesión LIVE en tu cuenta.
- Copiar el servidor y la clave. La clave está oculta por defecto.
- **Finalizar LIVE** desde la aplicación.
- Cerrar sesión. Opcionalmente, recordar la sesión en el Administrador de credenciales de Windows.
- **Copiar diagnóstico**, sin tokens ni claves, para informar de problemas.

## Instalación

1. Descarga `TikTok-OBS-Connector-Setup.exe`, ya sea desde los artefactos de la [CI de GitHub Actions](../../actions) o compilándolo tú (ver más abajo). El instalador local queda en `dist/installer/`.
2. Ejecútalo. Se instala para el usuario actual, sin permisos de administrador, e instala WebView2 si falta.
3. El instalador no está firmado: si Windows SmartScreen avisa, pulsa «Más información → Ejecutar de todas formas».

**Requisitos:** Windows 10/11 de 64 bits, OBS Studio y una cuenta de TikTok con acceso a LIVE mediante Streamlabs.

## Uso

1. Pulsa **«Iniciar sesión con TikTok»** y completa el login en el navegador.
2. Vuelve a la aplicación: verás tu cuenta y su estado de acceso a LIVE.
3. Escribe el **título**, elige la **categoría** y pulsa **«Obtener servidor y clave»**.
4. En OBS: **Ajustes → Emisión → Servicio: Personalizado** → pega el **Servidor** y la **Clave de retransmisión** → **Iniciar transmisión**.
5. Al terminar, detén la transmisión en OBS y pulsa **«Finalizar LIVE»**.

La clave cambia en cada LIVE.

## Cómo funciona

| Paso | Endpoint de Streamlabs |
|---|---|
| Login | `https://streamlabs.com/slobs/login?…&tiktok&…&port=<p>&code_challenge=<c>&code_flow=true` → vuelve a `http://127.0.0.1:<p>/?success=true&code=…` |
| Intercambio | `GET /api/v5/slobs/auth/data?code_verifier=…&code=…` → `data.oauth_token` |
| Acceso a LIVE | `GET /api/v5/slobs/tiktok/info` (Bearer) |
| Categorías | `GET /api/v5/slobs/tiktok/info?category=…` |
| Crear LIVE | `POST /api/v5/slobs/tiktok/stream/start` (multipart: `title`, `device_platform`, `category`, `audience_type`) → `{ id, rtmp, key }` |
| Finalizar | `POST /api/v5/slobs/tiktok/stream/{id}/end` |

El contrato se verificó el 4 de noviembre de 2026 a partir de dos fuentes: el código abierto de [Streamlabs Desktop](https://github.com/streamlabs/desktop) y el proyecto de referencia [StreamLabsTikTokStreamKeyGenerator](https://github.com/Loukious/StreamLabsTikTokStreamKeyGenerator). Todo lo específico de Streamlabs está aislado en `src-tauri/src/provider/streamlabs/`: si el API cambia, solo hay que tocar esa carpeta.

### Permiso de emisión

La app lee `reason`, `can_be_live` y `application_status.status` de `/tiktok/info` con **tipos estrictos**. Un campo ausente, nulo o con un tipo inesperado da un estado **desconocido**, nunca «no aprobada».

| `reason` \ `can_be_live` | `true` | `false` | ausente o inválido |
|---|---|---|---|
| `1` (aprobada) | confirmado | contradictorio | confirmado (Streamlabs Desktop solo usa `reason`) |
| `0`, `never_applied`, `rejected` | contradictorio | no disponible | no disponible |
| `2` (legacy) o `-1` (volver a entrar) | no disponible | no disponible | no disponible |
| ausente o inválido | confirmado (la referencia usa `can_be_live`) | no disponible | **desconocido** |

Streamlabs Desktop solo pide credenciales con la cuenta aprobada, y esta app hace lo mismo. Cuando el proveedor devuelve un error genérico (por ejemplo, `Unable to go live. Please try again later`), la app muestra su mensaje original y el código HTTP **sin afirmar una causa**. Solo dice «sin permiso» si Streamlabs envía un código explícito (`user_has_no_live_auth`, `scope_not_authorized`, `scope_permission_missed`).

**Diagnóstico:** `%LOCALAPPDATA%\com.tiktokobsconnector.app\logs\diagnostico.log` registra la operación, el código HTTP, el tipo de contenido, la duración, los campos de estado y los errores saneados. Nunca registra tokens, claves, cookies, cuerpos completos ni URLs con credenciales.

La aplicación se identifica con su propio User-Agent y con la plataforma real (`device_platform=win32`, el mismo valor que envía Streamlabs Desktop en Windows).

## Seguridad

- El **token** de Streamlabs y la **clave** viven solo en Rust, en un tipo `Secret`: no se serializan, se muestran como `••••` y su memoria se borra al liberarse. El token **nunca** llega a la interfaz.
- «Copiar» escribe en el portapapeles desde Rust. La clave completa solo se envía a la pantalla al pulsar «Mostrar».
- **Sesión:** solo en memoria, salvo que marques «Recordar». En ese caso se guarda en el **Administrador de credenciales de Windows**, nunca en archivos.
- **Servidor de retorno del login:**
  - Escucha solo en `127.0.0.1`, en un puerto aleatorio.
  - Valida la cabecera `Host` y el método.
  - Acepta una sola respuesta.
  - Se cierra al completar, cancelar o tras 5 minutos.
- Los registros eliminan tokens, códigos, verifiers y claves.
- **HTTP:** solo HTTPS (rustls) y sin seguir redirecciones.
- **Tauri:** CSP estricta y permisos mínimos.

## Desarrollo

Requisitos: Rust estable (≥ 1.85), Node.js ≥ 20 y, en Windows, las [dependencias de Tauri](https://v2.tauri.app/start/prerequisites/) (MSVC Build Tools y WebView2).

```bash
npm install
npm run tauri dev            # desarrollo
npm test                     # pruebas del frontend (Vitest)
cd src-tauri && cargo test   # pruebas de Rust (sin red)
cd src-tauri && cargo test --test streamlabs_contract -- --ignored   # contrato con Streamlabs real (Internet)
```

### Compilar el instalador

```bash
# En Windows
npm ci
npx tauri build
bash scripts/package-dist.sh            # → dist/installer/TikTok-OBS-Connector-Setup.exe

# Compilación cruzada desde WSL/Linux (cargo-xwin, clang, lld, llvm-rc, nsis y target x86_64-pc-windows-msvc)
npx tauri build --runner cargo-xwin --target x86_64-pc-windows-msvc
bash scripts/package-dist.sh x86_64-pc-windows-msvc
```

GitHub Actions (`.github/workflows/build-windows.yml`) ejecuta las pruebas y compila el instalador en cada push a `main` o `develop`, y lo publica como artefacto.

### Ramas

- `main`: versión estable.
- `develop`: integración del trabajo en curso. Las ramas de funcionalidad salen de aquí y vuelven con pull request.

### Estructura

```text
src/                          Interfaz React (AccountCard, StreamForm, CredentialsCard)
src-tauri/src/
  app_core.rs                 Orquestación: login, sesión, LIVE
  auth/                       PKCE y servidor de retorno en 127.0.0.1
  provider/streamlabs/        Contrato con Streamlabs (endpoints, modelos, errores)
  security/                   Secret, redacción y almacén de credenciales
  commands.rs                 Comandos Tauri
```

## Licencia

**GPL-3.0-or-later** (ver [LICENSE](LICENSE)). La implementación es propia y no copia código del proyecto de referencia. Sí se basa en los datos del protocolo documentados por proyectos GPL-3.0 (Streamlabs Desktop y StreamLabsTikTokStreamKeyGenerator). «TikTok» y «Streamlabs» son marcas de sus propietarios.
