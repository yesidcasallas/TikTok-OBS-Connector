# Acceso a TikTok LIVE: estados y evidencias

Hay cuatro estados distintos, y uno no implica el siguiente.

| Estado | ¿Qué significa? | Evidencia real del proyecto | Situación actual |
|---|---|---|---|
| **1. Inicio de sesión completado** | El usuario autorizó a una app en TikTok | Login real de `@yesidcasallasx` mediante **Streamlabs** (5 de noviembre de 2026). El diagnóstico registra `tiktok_info: HTTP 200 … user=presente` | **Sí**, mediante la integración de Streamlabs. Virelo no tiene app propia |
| **2. App aprobada para productos y permisos** | TikTok aprobó una app en el portal de desarrolladores para ciertos productos y scopes | **Virelo:** no hay app registrada (sin `client_key`). **Streamlabs:** su app está en producción, y Streamlabs Desktop declara scopes `live.room.info` y `live.room.manage` en `app/services/platforms/tiktok/api.ts`. Esos scopes **no** están en la lista pública de TikTok | Virelo: **no**. Streamlabs: sí, pero no es de Virelo |
| **3. Cuenta del creador habilitada para emitir** | TikTok concedió a la cuenta el acceso a LIVE desde PC para una integración concreta | Respuesta real de Streamlabs: `reason=0`, `can_be_live=false`, `application_status=never_applied`. La página de TikTok de la solicitud (`…/result/…?id=GL6399433079641606942`) muestra «Acceso en revisión · Streamlabs · En revisión» | **No**: solicitud en revisión. Streamlabs no refleja ese estado (sigue respondiendo `never_applied`) |
| **4. Integración habilitada para crear emisiones y obtener RTMP y clave** | La integración puede llamar a la creación del LIVE y recibir servidor y clave | **Streamlabs:** `POST …/tiktok/stream/start` existe (contrato verificado con token inválido → 401), pero **nunca se ha visto una respuesta correcta con esta cuenta**. **Virelo:** no hay ningún producto ni scope público que lo permita | **No verificado** con Streamlabs y **no disponible** para una integración propia |

## Lo que no debe confundirse

- **«Live» en el portal de desarrolladores** solo indica que una revisión de la app fue aprobada. No da permiso para transmitir ni habilita LIVE.
- **Login Kit** autentica al usuario (`user.info.basic`). **Content Posting API** publica vídeos (`video.publish`, `video.upload`). **Ninguno** crea emisiones ni genera claves RTMP.
- **Los 22 scopes públicos** de TikTok (revisados el 5 de noviembre de 2026) no incluyen ninguno de LIVE, RTMP ni claves de emisión.

## Dependencia no documentada

Crear y finalizar emisiones y obtener el servidor RTMP y la clave desde una integración propia requiere una autorización de TikTok que **no está documentada públicamente**. Streamlabs la tiene como integración de partner: tiene un identificador de aplicación de acceso propio (`GL6399433079641606942`) y StreamElements otro (`SE7403650104517334022`).

En este proyecto no se inventan endpoints, scopes, identificadores de partner ni aprobaciones. El paso siguiente es preguntar a TikTok (`consulta-tiktok-live.md`).

## Mensajes de la interfaz

La app distingue cada caso sin afirmar causas que el proveedor no indica.

| Caso | Origen | Mensaje |
|---|---|---|
| Denegación explícita de permisos | Código del proveedor `user_has_no_live_auth`, `scope_not_authorized` o `scope_permission_missed` | `PERMISSION_DENIED`: «Streamlabs indica expresamente que la cuenta no tiene permiso…» |
| Solicitud rechazada | `application_status=rejected` en `/tiktok/info` | «Según Streamlabs, TikTok rechazó la solicitud anterior.» |
| Error genérico, como `Unable to go live. Please try again later` con HTTP 403 | Sin código explícito | `PROVIDER_FORBIDDEN`: «Streamlabs no autorizó la petición (HTTP 403). No indica el motivo… **No significa que tu solicitud de acceso haya sido rechazada**», más el mensaje original del proveedor |
| Error genérico con HTTP 400 o 200 (`success:false`) | Sin código explícito | `PROVIDER_REJECTED`: «Streamlabs no aceptó la petición (HTTP …)», más el mensaje original |
| Fallo técnico (red, 5xx o respuesta inesperada) | — | `NETWORK`, `PROVIDER_UNAVAILABLE` o `PROVIDER_UNEXPECTED`. El estado del acceso pasa a «desconocido», **nunca** a «no aprobada» |
| Campo ausente, nulo o con tipo inesperado | `/tiktok/info` | «Estado desconocido» |

Pruebas que lo garantizan:

- `generic_provider_error_is_not_presented_as_missing_permission`
- `generic_error_is_never_worded_as_rejected_application`
- `missing_null_or_wrong_types_are_unknown_not_rejected`
- `failed_check_is_unknown_not_rejected`
