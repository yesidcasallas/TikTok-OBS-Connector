import { AccountCard } from "./components/AccountCard";
import { CredentialsCard } from "./components/CredentialsCard";
import { StreamForm } from "./components/StreamForm";
import { Badge, Spinner } from "./components/ui";
import { useApp } from "./useApp";

function Toasts() {
  const { toasts, dismiss } = useApp();
  return (
    <div className="toasts" aria-live="polite">
      {toasts.map((t) => (
        <div key={t.id} className={`toast toast-${t.kind}`} role={t.kind === "error" ? "alert" : "status"}>
          <div>
            <strong>{t.title}</strong>
            {t.message && <p>{t.message}</p>}
          </div>
          <button type="button" className="link" onClick={() => dismiss(t.id)} aria-label="Cerrar aviso">
            ✕
          </button>
        </div>
      ))}
    </div>
  );
}

export default function App() {
  const { snapshot } = useApp();
  if (!snapshot) {
    return (
      <div className="loading">
        <Spinner /> Cargando…
      </div>
    );
  }
  const { auth, stream, busy } = snapshot;

  return (
    <div className="app">
      <header className="top">
        <h1>TikTok OBS Connector</h1>
        {auth.status === "signedIn" ? (
          <Badge tone="ok">Conectado</Badge>
        ) : auth.status === "pending" ? (
          <Badge tone="warn">Conectando…</Badge>
        ) : (
          <Badge tone="neutral">Desconectado</Badge>
        )}
      </header>

      {busy && (
        <div className="busy" role="status">
          <Spinner /> {busy}
        </div>
      )}

      <main>
        <AccountCard />
        {stream ? <CredentialsCard /> : <StreamForm />}
      </main>

      <footer>
        No afiliado a TikTok ni a Streamlabs. Iniciar sesión no garantiza el acceso a LIVE: lo decide TikTok.
      </footer>
      <Toasts />
    </div>
  );
}
