// Utilidades puras de presentación (con pruebas en text.test.ts).

import type { LiveAccess } from "../types";

export type Tone = "ok" | "warn" | "bad" | "neutral";

export interface AccessInfo {
  label: string;
  tone: Tone;
  description: string;
}

export function accessInfo(access: LiveAccess, canBeLive: boolean | null): AccessInfo {
  switch (access) {
    case "approved":
      return { label: "Acceso a LIVE aprobado", tone: "ok", description: "" };
    case "legacy":
      return {
        label: "Acceso legacy",
        tone: "warn",
        description: "Para este tipo de acceso, Streamlabs no genera la clave: obtenla en TikTok LIVE Producer.",
      };
    case "notApproved":
    case "neverApplied":
    case "rejected":
      return {
        label: "Sin acceso a LIVE",
        tone: "bad",
        description: "Streamlabs indica que esta cuenta no está aprobada para emitir en TikTok LIVE.",
      };
    case "relog":
      return {
        label: "Vuelve a iniciar sesión",
        tone: "warn",
        description: "Streamlabs pide volver a iniciar sesión para actualizar los permisos.",
      };
    default:
      return {
        label: "Acceso a LIVE sin confirmar",
        tone: canBeLive === false ? "bad" : "neutral",
        description: "Streamlabs no indicó el estado del acceso. Si no hay permiso, se mostrará el motivo.",
      };
  }
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
