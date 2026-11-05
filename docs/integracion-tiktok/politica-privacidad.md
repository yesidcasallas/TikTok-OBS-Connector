# Política de privacidad de Virelo (borrador)

> **Borrador sin validez legal.** Refleja el comportamiento real de Virelo `0.1.0-beta.2`. Antes de publicarlo hay que completar `[ENTRE CORCHETES]` y revisarlo con un abogado; en Colombia aplica la Ley 1581 de 2012 de protección de datos personales. Si se activa la integración propia con TikTok Login Kit y un servidor de Virelo, hay que actualizar las secciones marcadas con ⚑.

**Responsable:** `[TITULAR]`, `[DIRECCIÓN]`, `[CONTACTO]`
**Última actualización:** `[FECHA]`

## 1. Qué es Virelo

Virelo es una aplicación de escritorio para Windows, macOS y Linux que ayuda a emitir en TikTok LIVE con OBS Studio. Hoy funciona con la **integración de Streamlabs**: iniciar sesión, consultar si tu cuenta puede emitir, crear o finalizar la sesión LIVE y obtener el servidor y la clave de transmisión se hacen a través de Streamlabs. Virelo no está afiliado a TikTok, OBS Project ni Streamlabs.

## 2. Datos que trata Virelo y dónde quedan

Virelo **no tiene servidores propios** ⚑: todo lo que se indica a continuación queda **en tu ordenador**.

| Dato | Para qué | Dónde y cuánto tiempo |
|---|---|---|
| Token de sesión de Streamlabs | Hacer las peticiones a Streamlabs en tu nombre | En memoria mientras la app está abierta. Solo si marcas «Recordar la sesión», en el almacén de credenciales del sistema (Administrador de credenciales de Windows, Keychain o Secret Service) hasta que cierres la sesión |
| Nombre de usuario de TikTok y estado del acceso a LIVE (por ejemplo, `reason`, `can_be_live` o `application_status`) | Mostrarte con qué cuenta trabajas y si puede emitir | En memoria. También aparecen en el registro de diagnóstico local (sección 4) |
| Servidor RTMP y clave de transmisión | Que los copies en OBS | **Solo en memoria.** Nunca se guardan en disco. Se copian al portapapeles cuando pulsas «Copiar» |
| Preferencias: recordar sesión, último título y categoría, tipo de audiencia, identificador y título del LIVE activo | Recordar tus opciones y poder finalizar un LIVE tras reiniciar | Archivo `preferences.json` en la carpeta de configuración de la app, hasta que lo borres o desinstales |
| Registro de diagnóstico | Ayudarte a resolver problemas | Archivo `diagnostico.log` local, de 512 KiB como máximo más una copia anterior. No contiene tokens, claves, códigos de autorización ni cookies |

Virelo **no** recoge contraseñas: tus credenciales de TikTok se escriben solo en la página de Streamlabs o TikTok, en tu navegador. Tampoco recoge contactos, ubicación, vídeos, mensajes, datos de pago ni estadísticas de uso, y **no incluye analítica, publicidad ni telemetría**.

## 3. Con quién se comunica Virelo

- **Streamlabs** (`streamlabs.com`): inicio de sesión y API para el estado del acceso, la creación y el fin del LIVE y la búsqueda de categorías. Se aplica la [política de privacidad de Streamlabs](https://streamlabs.com/privacy).
- **TikTok** (`tiktok.com`): Virelo abre en tu navegador las páginas oficiales de inicio de sesión y de solicitud de acceso a LIVE. Se aplica la [política de privacidad de TikTok](https://www.tiktok.com/legal/privacy-policy).
- **GitHub**: solo si descargas Virelo o sus actualizaciones desde GitHub Releases.

Virelo no vende ni comparte tus datos con terceros.

## 4. Registro de diagnóstico

Registra la operación, el código HTTP, el tipo de contenido, la duración, los campos de estado del acceso y los mensajes de error sin datos sensibles. También puede contener tu nombre de usuario de TikTok. Solo sale de tu ordenador si tú lo compartes, por ejemplo con «Copiar diagnóstico» al informar de un error.

Ubicaciones:

- **Windows:** `%LOCALAPPDATA%\com.virelo.desktop\logs\diagnostico.log`
- **macOS:** `~/Library/Logs/com.virelo.desktop/diagnostico.log`
- **Linux:** `~/.local/share/com.virelo.desktop/logs/diagnostico.log`

## 5. Tus derechos y cómo borrar tus datos

- **«Cerrar sesión»** borra el token de la app y del almacén de credenciales.
- **Preferencias y registro:** borra la carpeta `com.virelo.desktop` de configuración y de registros, o desinstala la app.
- **Autorización a Streamlabs:** puedes revocarla desde tu cuenta de Streamlabs o de TikTok.
- Para consultas, rectificación o supresión: `[CONTACTO]`.

## 6. Menores

Virelo no está dirigido a menores de `[EDAD MÍNIMA]` años. TikTok exige una edad mínima para emitir en LIVE.

## 7. Cambios

Publicaremos los cambios de esta política en `https://[DOMINIO]/privacidad` con su fecha.
