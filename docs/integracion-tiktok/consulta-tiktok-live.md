# Borrador: consulta a TikTok sobre una integración LIVE propia

> **Borrador, no enviado.** Canal sugerido: formulario de soporte de TikTok for Developers ([Support](https://developers.tiktok.com/support)). Completa los datos `[ENTRE CORCHETES]` antes de enviarlo.

---

**Subject:** LIVE streaming integration for a third-party desktop app (OBS/RTMP) — requirements and availability

Hello TikTok for Developers team,

I am `[NOMBRE]`, on behalf of `[TITULAR]` (`[PAÍS]`). We develop **Virelo**, a desktop application for creators who stream to TikTok LIVE using OBS Studio. Website: `https://[DOMINIO]`. Contact: `[CONTACTO]`.

We would like to build an **official, approved integration** instead of relying on another company's integration. Specifically, we would like our app to:

1. Let a creator sign in with TikTok (Login Kit for Desktop, PKCE, loopback redirect).
2. **Create and end a LIVE session** on the creator's behalf, with a title and a game category.
3. **Obtain the RTMP server URL and stream key** for that session, so the creator can paste them into OBS (or so we can configure OBS automatically).
4. Check whether the creator's account is eligible to go LIVE from PC, and direct them to the official application page if not.

Questions:

1. Is there an API, product or partner program that allows third-party desktop apps to create or end LIVE sessions and obtain the RTMP URL and stream key? We could not find a related scope in the public scopes list.
2. If so, what are the requirements (organization type, minimum users, security review, commercial agreement) and how do we apply?
3. Does TikTok provide a dedicated "LIVE access" application ID for each approved integration, similar to the existing PC LIVE access applications for other streaming tools? How is it requested?
4. **Linux:** we plan to support Windows, macOS and **Linux** (x86_64). Are there any platform restrictions for LIVE integrations or for the `device_platform` reported when creating a LIVE session?
5. **Colombia:** is this integration, and LIVE access from PC for creators, available in **Colombia**? Are there country-specific requirements?
6. Which scopes would the app need, and would a sandbox be available to test the LIVE flow before review?

We will only request the permissions we need, store user tokens securely on our server, and never ask users for their TikTok password.

Thank you,
`[NOMBRE]` — `[CARGO]`, `[TITULAR]`

---

## Versión en español (referencia interna)

**Asunto:** Integración de TikTok LIVE para una app de escritorio de terceros (OBS/RTMP): requisitos y disponibilidad

Somos `[TITULAR]` (`[PAÍS]`) y desarrollamos **Virelo**, una app de escritorio para creadores que emiten en TikTok LIVE con OBS. Queremos una integración **propia y aprobada** que permita:

- iniciar sesión con Login Kit Desktop;
- crear y finalizar la sesión LIVE con título y categoría;
- obtener el servidor RTMP y la clave para OBS;
- comprobar si la cuenta puede emitir desde PC.

Preguntamos:

1. ¿Existe un API, producto o programa de partners que lo permita?
2. ¿Qué requisitos tiene y cómo se solicita?
3. ¿Hay un id de aplicación de acceso a LIVE por integración aprobada?
4. ¿Hay restricciones para **Linux**?
5. ¿Está disponible en **Colombia**?
6. ¿Qué scopes harían falta, y hay sandbox para el flujo LIVE?
