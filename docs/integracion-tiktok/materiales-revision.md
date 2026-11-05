# Materiales para la revisión de TikTok — Virelo

> Borradores. **No se ha enviado nada a TikTok.** Los campos `[ENTRE CORCHETES]` son datos pendientes del titular.

## Datos pendientes

| Dato | Valor |
|---|---|
| Titular de la app (persona u organización) | `[TITULAR]` (se recomienda una organización) |
| País del titular | `[PAÍS]` (previsto: Colombia) |
| Dominio del sitio oficial | `[DOMINIO]` (ej.: `virelo.app`, sin comprobar disponibilidad) |
| Correo de contacto y soporte | `[CONTACTO]` |
| Dirección postal (si la pide el portal o la ley) | `[DIRECCIÓN]` |
| Cuenta de desarrollador TikTok | `[CORREO DE LA CUENTA]` |

## Descripción del producto

**Nombre:** Virelo

**Descripción corta (ES):** Virelo te ayuda a emitir en TikTok LIVE desde OBS en tu ordenador: inicias sesión con tu cuenta de TikTok y la app prepara la conexión con OBS.

**Descripción corta (EN):** Virelo helps you go LIVE on TikTok from OBS on your computer: sign in with your TikTok account and the app prepares the OBS connection.

**Descripción para la pantalla de autorización (EN):** Virelo is a desktop app (Windows, macOS, Linux) for creators who stream with OBS. Signing in lets Virelo show which TikTok account is connected.

> Ajusta el texto a lo que la app haga con **su** integración en el momento de la revisión. Con solo Login Kit, la integración propia **identifica la cuenta**; obtener el servidor y la clave sigue dependiendo de Streamlabs o de un acceso LIVE que TikTok no ofrece públicamente (ver `acceso-live.md`).

**Utilidad de la integración:** el creador ve en Virelo con qué cuenta de TikTok está trabajando. Virelo puede asociar la solicitud de acceso, las preferencias y el diagnóstico a esa cuenta, sin que el usuario escriba su contraseña en Virelo.

## Productos y permisos solicitados

| Producto o scope | ¿Se solicita? | Justificación (para el portal, EN) |
|---|---|---|
| Login Kit (Desktop) | **Sí** | "Virelo uses Login Kit so creators can sign in with their TikTok account in the system browser (PKCE, loopback redirect). We never ask for or store their TikTok password." |
| `user.info.basic` | **Sí** | "We display the connected account's avatar and display name inside the app, so the creator can confirm which TikTok account is linked before configuring OBS." |
| `user.info.profile` y `user.info.stats` | No | No se usan. |
| `video.list`, `video.upload` y `video.publish` (Content Posting API) | No | Virelo no lee ni publica vídeos. |
| Share Kit, Display API, Research API, Data Portability y Commercial Content | No | No se usan. |
| LIVE (crear o finalizar emisiones, RTMP) | **No existe como scope público** | Ver `consulta-tiktok-live.md`. |

Regla de la guía: «Only request permissions and features that your app needs». Si no se usa un producto o scope, hay que quitarlo antes de la revisión.

## Guion de la demo en el sandbox

Requisitos: sandbox creado, una cuenta propia añadida como usuario de prueba (máximo 10), Virelo compilado con el `client_key` del sandbox y el servidor intermediario desplegado. Si la app no se ha aprobado antes, la demo **debe** grabarse en el sandbox.

1. **00:00** Abrir la web oficial (`https://[DOMINIO]`) y mostrar los enlaces visibles a la Política de privacidad y a los Términos.
2. **00:15** Abrir Virelo en el escritorio. Debe verse el nombre y el icono de Virelo en la ventana.
3. **00:25** Pulsar «Iniciar sesión con TikTok». Se abre el navegador del sistema en `https://www.tiktok.com/v2/auth/authorize/…`.
4. **00:35** En la pantalla de autorización de TikTok deben verse **el nombre «Virelo», su icono y su descripción**, y el permiso «Read your profile info (avatar, display name)».
5. **00:45** Iniciar sesión con la cuenta de prueba y autorizar.
6. **00:55** El navegador vuelve a `http://127.0.0.1:<puerto>/callback/` y la página indica que se puede volver a Virelo.
7. **01:05** En Virelo se ven el avatar y el nombre visible de la cuenta (uso de `user.info.basic`).
8. **01:15** Pulsar «Cerrar sesión» y mostrar que la cuenta desaparece de la app.
9. **01:25** (Opcional) Mostrar el flujo cancelado: al denegar la autorización, Virelo muestra el error sin colgarse.

No mostrar en esta demo las funciones que dependen de Streamlabs. La demo debe mostrar **solo** los productos y scopes solicitados a TikTok.

## Evidencias necesarias para la revisión

- [ ] Cuenta de desarrollador y organización creadas (capturas).
- [ ] App «Virelo» registrada: nombre, descripción, categoría e icono de 1024 × 1024 (`recursos/virelo-icono-1024.png`).
- [ ] Sitio publicado en `https://[DOMINIO]`, con descripción del producto y enlaces visibles a Privacidad y Términos.
- [ ] URL de Términos, Privacidad y sitio **verificadas** en el portal, por dominio o por prefijo de URL.
- [ ] Plataforma Desktop configurada con la URL del sitio.
- [ ] Login Kit con el redirect URI `http://127.0.0.1:*/callback/`.
- [ ] Solo el scope `user.info.basic`.
- [ ] Servidor intermediario desplegado (HTTPS), con el `client_secret` en un almacén de secretos.
- [ ] Sandbox con un usuario de prueba y la demo grabada (MP4, hasta 50 MB).
- [ ] Explicación por producto y scope (la tabla de arriba).
- [ ] Versión estable de Virelo (no beta) disponible para descargar.
