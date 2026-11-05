// Cuenta: iniciar sesión con TikTok mediante Streamlabs, estado y cierre de sesión.

import { api } from "../api";
import { accessInfo, formatCountdown } from "../lib/text";
import { useApp, useNow } from "../useApp";
import { Badge, Button, Callout, Card, Spinner } from "./ui";

export function AccountCard() {
  const { snapshot, run, pending } = useApp();
  const now = useNow();
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
      <Card title="Cuenta" aside={<Badge tone="neutral">Sin sesión</Badge>}>
        <p className="muted">
          Se abrirá el navegador con Streamlabs. Escribe tus credenciales de TikTok solo en esa página.
        </p>
        {auth.accountError && <Callout tone="bad">{auth.accountError.message}</Callout>}
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

  const account = auth.account;
  const info = account ? accessInfo(account.liveAccess, account.canBeLive) : null;
  const name = account?.username || "cuenta vinculada";

  return (
    <Card title="Cuenta" aside={<Badge tone="ok">Conectada</Badge>}>
      <div className="account">
        <div className="avatar" aria-hidden>
          {name.charAt(0).toUpperCase()}
        </div>
        <div>
          <p className="lead">@{name}</p>
          {info && <p className={`small tone-${info.tone}`}>{info.label}</p>}
        </div>
        <div className="spacer" />
        <Button variant="ghost" onClick={() => run("logout", () => api.logout(), "Sesión cerrada")} loading={pending.logout}>
          Cerrar sesión
        </Button>
      </div>
      {info && info.tone !== "ok" && <p className="muted small">{info.description}</p>}
      {!account && auth.accountError && <Callout tone="warn">{auth.accountError.message}</Callout>}
      {remember}
    </Card>
  );
}
