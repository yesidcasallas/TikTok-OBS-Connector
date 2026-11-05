# Integración de Virelo con TikTok

Revisión del 5 de noviembre de 2026 sobre la preparación de Virelo (antes «TikTok OBS Connector») para una integración propia con TikTok. **No se ha presentado ninguna solicitud ni se ha publicado nada.**

| Documento | Contenido |
|---|---|
| [matriz-requisitos.md](matriz-requisitos.md) | Requisitos oficiales con estado, evidencia y acción pendiente |
| [oauth-y-credenciales.md](oauth-y-credenciales.md) | Flujo actual (Streamlabs), Login Kit preparado y tipos de credencial |
| [acceso-live.md](acceso-live.md) | Los cuatro estados del acceso a LIVE y su evidencia real |
| [materiales-revision.md](materiales-revision.md) | Descripción, justificación de permisos, guion de demo y evidencias |
| [consulta-tiktok-live.md](consulta-tiktok-live.md) | Borrador de consulta a TikTok sobre LIVE propio (Linux, Colombia) |
| [politica-privacidad.md](politica-privacidad.md), [terminos.md](terminos.md) | Borradores legales (generan `site/privacidad.html` y `site/terminos.html`) |
| [recursos/virelo-icono-1024.png](recursos/virelo-icono-1024.png) | Icono para el portal (1024 × 1024 PNG) |

## Situación actual

- **Integraciones:** Virelo funciona **solo** con la integración de **Streamlabs**, que se usa para el login, el estado del acceso, la creación y el fin del LIVE y la obtención de RTMP y clave. StreamElements no se usa: solo está centralizado su identificador.
- **Credenciales propias:** Virelo **no tiene credenciales propias de TikTok** (sin `client_key` ni app registrada). La pantalla de autorización muestra a Streamlabs, y la app lo dice expresamente.
- **Integración propia:** el código de Login Kit Desktop está **preparado y probado, pero desactivado** (`src-tauri/src/provider/tiktok_login_kit.rs`).

## Conclusión: preparación para Login Kit

**Parcialmente preparada. No se puede solicitar todavía.**

- **Listo en el repositorio:**
  - Nombre conforme (Virelo).
  - Icono original con las especificaciones.
  - Flujo de escritorio según la documentación vigente: PKCE con challenge hexadecimal, `state` y callback en loopback.
  - Ningún secreto en el cliente.
  - Justificación de `user.info.basic`, guion de la demo y borradores legales.
- **Bloqueos externos, sin los que la revisión se rechazaría:**
  - Titular y cuenta de desarrollador.
  - Dominio y sitio publicado con Privacidad y Términos visibles y verificados.
  - Servidor intermediario que guarde el `client_secret`.
  - Sandbox y vídeo de demo.
  - Una versión estable, porque la guía rechaza apps en desarrollo.

## Conclusión: integración LIVE

**No disponible para una integración propia con la información pública actual.** Login Kit y Content Posting API no crean emisiones ni generan claves RTMP, y ninguno de los 22 scopes públicos cubre LIVE.

Hoy la creación del LIVE solo es posible a través de Streamlabs y **no está verificada** con la cuenta de prueba: su solicitud de acceso a LIVE desde PC está «En revisión» en TikTok. Una integración LIVE propia depende de una autorización de TikTok no documentada públicamente; el siguiente paso es la consulta de [consulta-tiktok-live.md](consulta-tiktok-live.md).

**Nada de lo anterior significa que Virelo esté aprobado por TikTok ni que pueda obtener claves RTMP por sí mismo.**

## Pendientes por prioridad

1. **Titular y cuenta:** definir el titular (preferible una organización), crear la cuenta de TikTok for Developers y completar `[TITULAR]`, `[CONTACTO]` y `[PAÍS]`.
2. **Dominio y sitio:** registrar el dominio, completar y publicar `site/` (regenerar con `python3 scripts/generar-sitio.py`) y verificar el dominio en el portal.
3. **Revisión legal:** de la Política de privacidad y los Términos (Ley 1581 de 2012).
4. **Consulta LIVE:** enviar la consulta a TikTok (el resultado decide si una integración propia tiene sentido).
5. **Servidor intermediario:** desarrollar y desplegar el intercambio y la renovación de tokens con `client_secret` (contrato en `oauth-y-credenciales.md`).
6. **App y Login Kit:** registrar «Virelo» en el portal (Desktop + Login Kit + `user.info.basic` + redirect `http://127.0.0.1:*/callback/`) y compilar con `VIRELO_TIKTOK_CLIENT_KEY` y `VIRELO_TOKEN_BROKER_URL`.
7. **Interfaz de Login Kit:** conectar el flujo propio a la app, para mostrar el avatar y el nombre. Requiere permitir en la CSP las imágenes del CDN de TikTok o servirlas desde el servidor.
8. **Sandbox y demo:** crear el sandbox con un usuario de prueba y grabar la demo.
9. **Versión estable:** publicar una versión no beta y enviar a revisión.
10. **Repositorio:** renombrar en GitHub `TikTok-OBS-Connector` a algo coherente con Virelo, que es una acción externa no realizada, y comprobar la marca «Virelo».
