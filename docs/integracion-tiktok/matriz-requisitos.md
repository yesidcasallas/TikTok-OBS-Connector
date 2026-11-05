# Matriz de requisitos de TikTok for Developers — Virelo

Revisión del 5 de noviembre de 2026, basada en la documentación oficial vigente:

- [Create an app](https://developers.tiktok.com/docs/en/getting-started-create-an-app)
- [App review guidelines](https://developers.tiktok.com/docs/en/app-review-guidelines)
- [Developer guidelines](https://developers.tiktok.com/docs/en/our-guidelines-developer-guidelines)
- [Login Kit for Desktop](https://developers.tiktok.com/docs/en/login-kit-desktop)
- [Scopes](https://developers.tiktok.com/docs/en/tiktok-api-scopes)
- [Sandbox](https://developers.tiktok.com/docs/en/add-a-sandbox)
- [FAQ](https://developers.tiktok.com/docs/en/getting-started-faq)
- [Get a user access token](https://developers.tiktok.com/docs/en/oauth-get-a-user-access-token)

**Estados:**

- **Cumple:** hay evidencia en el repositorio.
- **Parcial:** preparado pero incompleto.
- **No cumple:** falta y es necesario.
- **No verificable:** requiere acceso al portal de desarrolladores o una comprobación externa que no se ha hecho.

| # | Requisito | Estado | Evidencia | Archivo o URL | Acción pendiente |
|---|---|---|---|---|---|
| 1 | Cuenta de desarrollador en TikTok for Developers | No verificable | No hay acceso al portal ni capturas de la cuenta. | developers.tiktok.com | Crear o confirmar la cuenta con el correo del titular y guardar una captura. |
| 2 | Propietario de la app: se recomienda una **organización**; registrarla como persona individual «no se recomienda para integraciones reales» | No verificable | Faltan los datos del titular. | `materiales-revision.md` § Datos pendientes | Decidir el titular (persona u organización), crear la organización en el portal y completar `[TITULAR]`. |
| 3 | App registrada en el portal (Draft → In review → Live) | No verificable | El repositorio no tiene `client_key`: `LoginKitConfig::from_build_env()` devuelve `None`. | `src-tauri/src/provider/tiktok_login_kit.rs` | Registrar «Virelo» en el portal y guardar el `client_key` como variable de compilación (`VIRELO_TIKTOK_CLIENT_KEY`). |
| 4 | Nombre propio, sin referencias a redes sociales («TikTok app») y que coincida con la app o el sitio | Cumple | El nombre visible pasó de «TikTok OBS Connector», que incumplía la guía, a **Virelo** en la interfaz, los metadatos y los paquetes. | `src-tauri/tauri.conf.json`, `src/App.tsx`, `README.md` | Buscar marcas registradas con el nombre «Virelo» en los países de distribución. |
| 5 | Descripción visible en la pantalla de autorización que explique qué hace la app | Parcial | Hay textos preparados en español e inglés. | `materiales-revision.md` § Descripción | Copiarlos en el portal cuando exista la app. |
| 6 | Categoría | No verificable | El portal no publica la lista de categorías. | — | Elegir en el portal la categoría más cercana (propuesta: herramientas para creadores o streaming). |
| 7 | Icono de 1024 × 1024 px, JPEG o PNG, hasta 5 MB, claro, coherente con la marca y que no se confunda con otra marca | Cumple | Icono original nuevo, sin los colores de TikTok (el anterior usaba `#25f4ee` y `#fe2c55`). PNG de 1024 × 1024, 54 KB. | `docs/integracion-tiktok/recursos/virelo-icono-1024.png`, `app-icon.svg` | — |
| 8 | App funcional, destinada a otros usuarios y no «en desarrollo o pruebas» | Parcial | Probados con una cuenta real: login, comprobación de acceso y solicitud de acceso. Es una beta (`0.1.0-beta.2`), y obtener servidor y clave no se ha verificado con una cuenta aprobada. | `.github/releases/v0.1.0-beta.1.md`, `acceso-live.md` | Publicar una versión estable antes de la revisión; la guía rechaza apps en desarrollo. |
| 9 | Sitio web oficial, desarrollado, que describa el producto (no una página de aterrizaje ni de login) | No cumple | No hay sitio publicado. Existe un borrador local. | `site/index.html` | Comprar el dominio, completar el contenido y publicarlo. |
| 10 | Política de privacidad y Términos visibles en el sitio, sin abrir menús, y activos | No cumple | Hay borradores que reflejan el comportamiento real, pendientes de datos del titular y de revisión legal. | `politica-privacidad.md`, `terminos.md`, `site/privacidad.html`, `site/terminos.html` | Completar `[TITULAR]`, `[DOMINIO]` y `[CONTACTO]`, revisarlos con un abogado (Ley 1581 de 2012, Colombia) y publicarlos. |
| 11 | Verificar en el portal las URL de Términos, Privacidad y sitio (por dominio o por prefijo de URL) | No cumple | No hay dominio. | — | Tras publicar el sitio, verificar el dominio desde el portal. |
| 12 | Plataforma Desktop con la URL oficial del sitio | No verificable | Requiere el portal y el sitio publicado. | — | Configurar la plataforma Desktop con la URL del sitio. |
| 13 | Login Kit Desktop: redirect URI solo con `localhost` o `127.0.0.1`, con puerto (comodín permitido), estático y sin parámetros | Parcial | El código usa `http://127.0.0.1:*/callback/` y un servidor de retorno solo en loopback. Falta registrarlo en el portal. | `tiktok_login_kit.rs` (`REGISTERED_REDIRECT_URI`), `src-tauri/src/auth/callback.rs` | Registrar `http://127.0.0.1:*/callback/` en Login Kit. |
| 14 | PKCE de escritorio: verifier de 43 a 128 caracteres; challenge en **SHA-256 hexadecimal** con `S256`; `state` anti-CSRF validado | Cumple (preparado) | Implementado y probado en la integración propia: challenge hexadecimal con vector FIPS, `state` de 256 bits comparado en tiempo constante. | `tiktok_login_kit.rs` (pruebas) | Conectarlo a la interfaz cuando existan las credenciales. |
| 15 | `client_secret` solo en el servidor; llamar a `/v2/oauth/token/` desde un servidor, no desde código cliente | Parcial | El ejecutable no contiene ni pide ningún secreto (hay una prueba). **No existe todavía** el servidor intermediario. | `tiktok_login_kit.rs`, `oauth-y-credenciales.md` | Desarrollar y alojar el servidor intermediario con el contrato documentado. |
| 16 | Solicitar solo los productos y scopes necesarios, con su justificación | Parcial | Previsto: Login Kit + `user.info.basic`, con justificación redactada. No se piden scopes de vídeo ni de datos. | `materiales-revision.md` § Permisos | Añadir solo esos en el portal. |
| 17 | Sandbox: hasta 5 sandboxes y hasta 10 usuarios de prueba propios; obligatorio para la demo si la app no se aprobó antes | No verificable | Requiere el portal. Hay un guion de demo preparado. | `materiales-revision.md` § Guion | Crear el sandbox, añadir usuarios de prueba y grabar la demo. |
| 18 | Materiales de revisión: explicación por producto y scope y al menos un vídeo completo (hasta 5, de 50 MB cada uno); el dominio del vídeo debe coincidir con el sitio | Parcial | El texto está redactado. Faltan los vídeos y el dominio. | `materiales-revision.md` | Grabar el vídeo en el sandbox con el sitio publicado. |
| 19 | No dar a entender afiliación con TikTok u otras entidades; comunicación no engañosa | Parcial | La interfaz, el README y los paquetes avisan de que Virelo no está afiliado a TikTok, OBS ni Streamlabs, y la pantalla de login aclara que la autorización la concede Streamlabs. La dependencia de endpoints internos de Streamlabs sigue siendo un riesgo de cumplimiento con Streamlabs. | `src/components/AccountCard.tsx`, `src/App.tsx`, `README.md` | Decidir si se mantiene la dependencia de Streamlabs tras obtener la integración propia. |
| 20 | Proteger los datos del usuario y tratar la seguridad con urgencia | Cumple | Los tokens viven en `Secret` (sin `Serialize`, borrado con `zeroize`), se guardan solo en el llavero del sistema y se redactan en los registros. Hay pruebas que impiden su filtrado. | `src-tauri/src/security/`, `app_core/tests.rs` | — |
| 21 | Acceso a LIVE (crear o finalizar emisiones y obtener RTMP y clave) mediante productos o scopes públicos | No cumple (no disponible) | Ninguno de los 22 scopes públicos cubre LIVE, RTMP ni claves. El acceso depende de una autorización de TikTok no documentada públicamente. | [Scopes](https://developers.tiktok.com/docs/en/tiktok-api-scopes), `acceso-live.md` | Enviar la consulta de `consulta-tiktok-live.md`. |
| 22 | Revisión: «de varios días a dos semanas»; si no se aprueba, se ven los comentarios en *History* | No verificable | Aún no se ha enviado ninguna solicitud. | [FAQ](https://developers.tiktok.com/docs/en/getting-started-faq) | — |

## Resumen

- **Cumple:** 4 requisitos (nombre, icono, PKCE y `state` preparados, y protección de datos).
- **Parcial:** 7 requisitos. Casi todos tienen el código o el texto preparado y dependen del portal o del servidor intermediario.
- **No cumple:** 4 requisitos. Sitio, políticas publicadas, verificación de URL y acceso LIVE público.
- **No verificable:** 7 requisitos, todos dependientes del portal, de datos del titular o de una revisión aún no enviada.
