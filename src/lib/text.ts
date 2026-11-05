// Utilidades puras de presentación (con pruebas en text.test.ts).

import type { AccessView, ErrorInfo, UnavailableCause } from "../types";

export type Tone = "ok" | "warn" | "bad" | "neutral";

export interface AccessPresentation {
  label: string;
  tone: Tone;
  description: string;
  /** El botón «Obtener servidor y clave» solo se habilita con permiso confirmado. */
  canRequest: boolean;
  /** Ofrecer «Consultar estado» y «Solicitar acceso». */
  offerAccessLinks: boolean;
  checking: boolean;
}

const CAUSE_TEXT: Record<UnavailableCause, string> = {
  notApproved: "Según Streamlabs, tu cuenta no tiene acceso a LIVE desde PC.",
  neverApplied: "Streamlabs aún no registra tu acceso. Si ya enviaste la solicitud, consulta su estado en TikTok.",
  rejected: "Según Streamlabs, TikTok rechazó tu solicitud de acceso.",
  legacy: "Tu clave se obtiene en TikTok LIVE Producer, no con Streamlabs.",
  relog: "Cierra sesión y vuelve a entrar para actualizar los permisos.",
  cannotGoLive: "Según Streamlabs, ahora no puedes emitir.",
};

/** Valores documentados de `application_status.status` (`TTikTokApplicationStatus`). */
const KNOWN_APPLICATION_STATUS = ["approved", "rejected", "never_applied"];

export function accessPresentation(access: AccessView): AccessPresentation {
  const base = { canRequest: false, offerAccessLinks: false, checking: false };
  switch (access.state) {
    case "idle":
      return { ...base, label: "Acceso sin comprobar", tone: "neutral", description: "" };
    case "checking":
      return { ...base, checking: true, label: "Comprobando acceso…", tone: "neutral", description: "" };
    case "failed":
      return { ...base, label: "No se pudo comprobar el acceso", tone: "warn", description: access.error.message };
    case "done": {
      const a = access.account.access.availability;
      switch (a.kind) {
        case "confirmed":
          return { ...base, canRequest: true, label: "Acceso a LIVE confirmado", tone: "ok", description: "" };
        case "notAvailable": {
          const status = access.account.access.applicationStatus;
          const unrecognized =
            status.state === "present" && !KNOWN_APPLICATION_STATUS.includes(status.value) ? status.value : null;
          return {
            ...base,
            offerAccessLinks: a.cause !== "legacy" && a.cause !== "relog",
            label: "Sin acceso a LIVE",
            tone: "bad",
            description: unrecognized
              ? `Estado de la solicitud según Streamlabs: «${unrecognized}».`
              : CAUSE_TEXT[a.cause],
          };
        }
        case "contradictory":
        case "unknown":
          return {
            ...base,
            offerAccessLinks: true,
            label: "Acceso sin confirmar",
            tone: "warn",
            description: "Streamlabs no lo deja claro. Actualiza el estado.",
          };
      }
    }
  }
}

/** Texto del aviso de error: explicación propia + mensaje original del proveedor. */
export function errorText(e: ErrorInfo): string {
  const parts = [e.message];
  if (e.detail) parts.push(`Mensaje de Streamlabs: «${e.detail}»`);
  if (e.httpStatus && !e.message.includes(`HTTP ${e.httpStatus}`)) parts.push(`(HTTP ${e.httpStatus})`);
  return parts.join("\n");
}

const pad = (n: number) => String(n).padStart(2, "0");

/** mm:ss para cuentas atrás. */
export function formatCountdown(ms: number): string {
  if (!Number.isFinite(ms) || ms <= 0) return "00:00";
  const total = Math.ceil(ms / 1000);
  return `${pad(Math.floor(total / 60))}:${pad(total % 60)}`;
}

/** hh:mm:ss para tiempo transcurrido. */
export function formatElapsed(ms: number): string {
  if (!Number.isFinite(ms) || ms < 0) return "00:00:00";
  const total = Math.floor(ms / 1000);
  return `${pad(Math.floor(total / 3600))}:${pad(Math.floor((total % 3600) / 60))}:${pad(total % 60)}`;
}

/** Devuelve el mensaje de error del título o `null` si es válido. */
export function validateTitle(title: string, max: number): string | null {
  const t = title.trim();
  if (!t) return "Escribe un título para el LIVE.";
  if ([...t].length > max) return `Máximo ${max} caracteres.`;
  // eslint-disable-next-line no-control-regex
  if (/[\u0000-\u001f\u007f]/.test(t)) return "El título contiene caracteres no válidos.";
  return null;
}
