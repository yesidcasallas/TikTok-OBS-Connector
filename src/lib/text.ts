// Utilidades puras de presentación (con pruebas en text.test.ts).

import type { AccessView, ErrorInfo, UnavailableCause } from "../types";

export type Tone = "ok" | "warn" | "bad" | "neutral";

/**
 * Modo de la tarjeta de cuenta:
 * - `enabled`: permiso confirmado por el proveedor.
 * - `request`: sin permiso; se ofrece solicitar acceso.
 * - `pending`: el proveedor informa de una solicitud en revisión.
 * - `blocked`: sin permiso por un motivo que no se resuelve solicitando (legacy, volver a entrar).
 * - `unknown`: no se pudo determinar (fallo de comprobación o datos insuficientes).
 * - `checking` / `idle`.
 */
export type AccessMode = "enabled" | "request" | "pending" | "blocked" | "unknown" | "checking" | "idle";

export interface AccessPresentation {
  mode: AccessMode;
  label: string;
  tone: Tone;
  description: string;
  /** El formulario del LIVE solo se habilita con permiso confirmado. */
  canRequest: boolean;
  checking: boolean;
}

/** Mensaje principal cuando la sesión está conectada pero sin permiso de emisión. */
export const REQUEST_ACCESS_TEXT =
  "Tu cuenta está conectada. Solicita acceso a LIVE proporcionando la información y las capturas que TikTok solicite.";

const CAUSE_DETAIL: Record<UnavailableCause, string> = {
  notApproved: "Según Streamlabs, tu cuenta aún no tiene acceso a LIVE desde PC.",
  neverApplied: "Streamlabs aún no registra tu acceso. Si ya enviaste la solicitud, consúltala.",
  rejected: "Según Streamlabs, TikTok rechazó la solicitud anterior.",
  cannotGoLive: "Según Streamlabs, ahora no puedes emitir.",
  pending: "TikTok está revisando tu solicitud. La aprobación la decide TikTok.",
  legacy: "Tu clave se obtiene en TikTok LIVE Producer, no con Streamlabs.",
  relog: "Cierra sesión y vuelve a entrar para actualizar los permisos.",
};

/** Valores documentados de `application_status.status` (`TTikTokApplicationStatus`). */
const KNOWN_APPLICATION_STATUS = ["approved", "rejected", "never_applied"];
const PENDING_APPLICATION_STATUS = ["pending", "in_review", "under_review", "reviewing", "submitted", "processing"];

export function accessPresentation(access: AccessView): AccessPresentation {
  const base = { canRequest: false, checking: false };
  switch (access.state) {
    case "idle":
      return { ...base, mode: "idle", label: "Acceso sin comprobar", tone: "neutral", description: "" };
    case "checking":
      return { ...base, mode: "checking", checking: true, label: "Comprobando acceso…", tone: "neutral", description: "" };
    case "failed":
      return { ...base, mode: "unknown", label: "Estado desconocido", tone: "warn", description: access.error.message };
    case "done": {
      const a = access.account.access.availability;
      switch (a.kind) {
        case "confirmed":
          return { ...base, mode: "enabled", canRequest: true, label: "Acceso a LIVE habilitado", tone: "ok", description: "" };
        case "notAvailable": {
          if (a.cause === "pending") {
            return { ...base, mode: "pending", label: "Solicitud en revisión", tone: "warn", description: CAUSE_DETAIL.pending };
          }
          if (a.cause === "legacy" || a.cause === "relog") {
            return { ...base, mode: "blocked", label: "Emisión no disponible", tone: "bad", description: CAUSE_DETAIL[a.cause] };
          }
          const status = access.account.access.applicationStatus;
          const raw =
            status.state === "present" &&
            !KNOWN_APPLICATION_STATUS.includes(status.value) &&
            !PENDING_APPLICATION_STATUS.includes(status.value.toLowerCase())
              ? status.value
              : null;
          return {
            ...base,
            mode: "request",
            label: "Emisión no disponible",
            tone: "bad",
            description: raw ? `Estado de la solicitud según Streamlabs: «${raw}».` : CAUSE_DETAIL[a.cause],
          };
        }
        case "contradictory":
        case "unknown":
          return {
            ...base,
            mode: "unknown",
            label: "Estado desconocido",
            tone: "warn",
            description: "Streamlabs no lo deja claro. Actualiza el estado.",
          };
      }
    }
  }
}

/** Intervalo mínimo entre comprobaciones automáticas al recuperar el foco. */
export const FOCUS_REFRESH_MIN_MS = 30_000;

/**
 * Si conviene comprobar el permiso al volver a la aplicación: solo con sesión,
 * sin permiso confirmado, sin otra comprobación en curso y como mucho una vez
 * cada `FOCUS_REFRESH_MIN_MS` (contando también la última comprobación hecha).
 */
export function shouldRefreshOnFocus(
  signedIn: boolean,
  access: AccessView,
  lastAttemptMs: number,
  nowMs: number,
  minIntervalMs = FOCUS_REFRESH_MIN_MS,
): boolean {
  if (!signedIn || access.state === "checking") return false;
  if (access.state === "done" && access.account.access.availability.kind === "confirmed") return false;
  const lastCheck = access.state === "done" ? Math.max(lastAttemptMs, access.account.checkedAtMs) : lastAttemptMs;
  return nowMs - lastCheck >= minIntervalMs;
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
