// Cuenta: iniciar sesión con TikTok mediante Streamlabs, estado y cierre de sesión.

import { useState } from "react";
import { api } from "../api";
import { accessPresentation, errorText, formatCountdown, REQUEST_ACCESS_TEXT } from "../lib/text";
import { useApp, useNow } from "../useApp";
import { Badge, Button, Callout, Card, Spinner } from "./ui";

/** Guía breve antes de abrir el formulario oficial de TikTok. */
function RequestGuide({
  username,
  provider,
  onOpen,
  onCancel,
}: {
  username: string | null;
  provider: string;
  onOpen: () => void;
  onCancel: () => void;
}) {
  return (
    <div className="guide" role="group" aria-label="Cómo solicitar el acceso">
      <p className="lead">Antes de abrir TikTok</p>
      <ol className="steps">
        <li>
          Inicia sesión en TikTok con la misma cuenta{username ? <> (<strong>@{username}</strong>)</> : null}.
        </li>
        <li>Selecciona la plataforma donde ya transmites e introduce el enlace de tu canal.</li>
        <li>Completa los datos que pida TikTok.</li>
        <li>Adjunta las capturas que TikTok solicite para acreditar la titularidad o actividad del canal.</li>
        <li>Envía la solicitud y regresa a esta aplicación.</li>
      </ol>
      <p className="muted small">
        Todo se completa en el formulario oficial de TikTok para {provider}; sigue lo que te pida. Abrirlo no envía la
        solicitud, y la aprobación la decide TikTok.
      </p>
      <div className="row">
        <Button variant="primary" onClick={onOpen}>
          Abrir solicitud en TikTok
        </Button>
        <Button variant="ghost" onClick={onCancel}>
          Cancelar
        </Button>
      </div>
    </div>
  );
}

export function AccountCard() {
  const { snapshot, run, pending } = useApp();
  const now = useNow();
  const [guide, setGuide] = useState(false);
  if (!snapshot) return null;
  const { auth } = snapshot;

  const remember = (
    <label className="check">
      <input
        type="checkbox"
        checked={auth.remember}
        disabled={pending.remember}
        onChange={(e) => {
          const value = e.target.checked;
          run("remember", () => api.setRemember(value));
        }}
      />
      <span>
        Recordar la sesión en este equipo
        {auth.persistence.mode === "unavailable" && (
          <small>Almacén de credenciales no disponible: la sesión se mantiene solo en memoria.</small>
        )}
      </span>
    </label>
  );

  if (auth.status === "pending" && auth.pending) {
    const p = auth.pending;
    return (
      <Card title="Cuenta" aside={<Badge tone="warn">Esperando…</Badge>}>
        <div className="pending">
          <Spinner label="Esperando el inicio de sesión" />
          <p>
            Completa el inicio de sesión en el navegador.{" "}
            <span className="muted mono">{formatCountdown(p.expiresAtMs - now)}</span>
          </p>
        </div>
        {!p.browserOpened && (
          <Callout tone="warn" title="No se pudo abrir el navegador">
            Copia el enlace y ábrelo en el navegador de este equipo.
          </Callout>
        )}
        <div className="row">
          {!p.browserOpened && (
            <Button onClick={() => run("copyUrl", () => api.copy("loginUrl"), "Enlace copiado")}>Copiar enlace</Button>
          )}
          <Button variant="ghost" onClick={() => run("cancel", () => api.cancelLogin())}>
            Cancelar
          </Button>
        </div>
      </Card>
    );
  }

  if (auth.status !== "signedIn") {
    return (
      <Card title="Cuenta">
        <p className="muted">
          Se abrirá el navegador con Streamlabs. Escribe tus credenciales de TikTok solo en esa página.
        </p>
        {auth.sessionError && <Callout tone="bad">{errorText(auth.sessionError)}</Callout>}
        <div className="row">
          <Button
            variant="primary"
            loading={pending.login || !!snapshot.busy}
            onClick={() => run("login", () => api.login(), "Sesión iniciada")}
          >
            Iniciar sesión con TikTok
          </Button>
        </div>
        {remember}
      </Card>
    );
  }

  const access = accessPresentation(auth.access);
  const name = auth.username || "cuenta vinculada";
  const refresh = (
    <Button onClick={() => run("refresh", () => api.refreshAccount())} loading={pending.refresh || access.checking}>
      Actualizar estado
    </Button>
  );
  const consult = (
    <Button onClick={() => run("status", () => api.openAccessPage("status"))}>Consultar mi solicitud</Button>
  );

  return (
    <Card title="Cuenta">
      <div className="account">
        <div className="avatar" aria-hidden>
          {name.charAt(0).toUpperCase()}
        </div>
        <div>
          <p className="lead">@{name}</p>
          <p className={`small tone-${access.tone}`}>
            {access.checking && <Spinner label="Comprobando" />} {access.label}
          </p>
        </div>
        <div className="spacer" />
        <Button variant="ghost" onClick={() => run("logout", () => api.logout(), "Sesión cerrada")} loading={pending.logout}>
          Cerrar sesión
        </Button>
      </div>

      {access.mode === "request" && (
        <>
          <p>{REQUEST_ACCESS_TEXT}</p>
          {access.description && <p className="muted small">{access.description}</p>}
          {guide ? (
            <RequestGuide
              username={auth.username}
              provider={snapshot.app.provider}
              onOpen={() => {
                setGuide(false);
                run("apply", () => api.openAccessPage("apply"));
              }}
              onCancel={() => setGuide(false)}
            />
          ) : (
            <div className="row">
              <Button variant="primary" onClick={() => setGuide(true)}>
                Solicitar acceso a LIVE
              </Button>
              {consult}
              {refresh}
            </div>
          )}
        </>
      )}

      {access.mode === "pending" && (
        <>
          <p className="muted small">{access.description}</p>
          <div className="row">
            {consult}
            {refresh}
          </div>
        </>
      )}

      {(access.mode === "blocked" || access.mode === "unknown" || access.mode === "idle") && (
        <>
          {access.description && <p className="muted small">{access.description}</p>}
          <div className="row">
            {access.mode === "unknown" && consult}
            {refresh}
          </div>
        </>
      )}
      {remember}
    </Card>
  );
}
