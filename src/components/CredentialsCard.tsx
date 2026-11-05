// Servidor y clave (oculta por defecto), copiar y finalizar el LIVE.

import { useEffect, useState } from "react";
import { api, toErrorInfo } from "../api";
import { formatElapsed } from "../lib/text";
import { useApp, useNow } from "../useApp";
import { Badge, Button, Card, ConfirmButton } from "./ui";

export function CredentialsCard() {
  const { snapshot, run, pending, notify } = useApp();
  const now = useNow();
  const [revealed, setRevealed] = useState<string | null>(null);
  const stream = snapshot?.stream ?? null;

  useEffect(() => setRevealed(null), [stream?.id]);

  if (!snapshot || !stream) return null;
  const signedIn = snapshot.auth.status === "signedIn";

  const toggleKey = async () => {
    if (revealed) return setRevealed(null);
    try {
      setRevealed(await api.revealKey());
    } catch (err) {
      notify("error", "No se pudo mostrar la clave", toErrorInfo(err).message);
    }
  };

  const masked = "••••••••••••" + (stream.keyHint?.replace("••••", "") ?? "");

  return (
    <Card title="Servidor y clave" aside={<Badge tone="live">● LIVE · {formatElapsed(now - stream.startedAtMs)}</Badge>}>
      <p className="muted small">
        {stream.title} · {stream.categoryName}
      </p>

      {stream.restored ? (
        <p className="muted small">
          Este LIVE se creó en una sesión anterior: el servidor y la clave ya no se muestran aquí.
        </p>
      ) : (
        <>
          <div className="field">
            <label htmlFor="server">Servidor</label>
            <div className="copy-row">
              <input id="server" className="mono" readOnly value={stream.server ?? ""} />
              <Button onClick={() => run("copyServer", () => api.copy("server"), "Servidor copiado")}>Copiar</Button>
            </div>
          </div>
          <div className="field">
            <label htmlFor="key">Clave de transmisión</label>
            <div className="copy-row">
              <input id="key" className="mono" readOnly value={revealed ?? masked} />
              <Button onClick={toggleKey} disabled={!stream.keyAvailable}>
                {revealed ? "Ocultar" : "Mostrar"}
              </Button>
              <Button
                variant="primary"
                onClick={() => run("copyKey", () => api.copy("key"), "Clave copiada")}
                disabled={!stream.keyAvailable}
              >
                Copiar
              </Button>
            </div>
          </div>
          <p className="muted small">
            En OBS: <strong>Ajustes → Emisión → Servicio: Personalizado</strong> → pega el servidor y la clave.
          </p>
        </>
      )}

      <div className="row">
        <ConfirmButton
          variant="danger"
          disabled={!signedIn}
          loading={pending.end}
          question="Detén primero la transmisión en OBS. ¿Finalizar el LIVE en TikTok?"
          confirmLabel="Finalizar LIVE"
          onConfirm={() => run("end", () => api.endStream(), "LIVE finalizado")}
        >
          Finalizar LIVE
        </ConfirmButton>
        {!signedIn && <span className="muted small">Inicia sesión para poder finalizarlo.</span>}
      </div>
    </Card>
  );
}
