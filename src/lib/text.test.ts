import { describe, expect, it } from "vitest";
import type { AccessView, AccountInfo, Availability } from "../types";
import {
  accessPresentation,
  errorText,
  FOCUS_REFRESH_MIN_MS,
  formatCountdown,
  formatElapsed,
  REQUEST_ACCESS_TEXT,
  shouldRefreshOnFocus,
  validateTitle,
} from "./text";

function account(availability: Availability, extra: Partial<AccountInfo["access"]> = {}, checkedAtMs = 0): AccountInfo {
  return {
    username: "creadora",
    displayName: null,
    access: {
      reason: { state: "present", value: 0 },
      canBeLive: { state: "present", value: false },
      applicationStatus: { state: "present", value: "never_applied" },
      userPresent: true,
      scope: "neverApplied",
      availability,
      ...extra,
    },
    audience: { disabled: false, defaultValue: "0", options: [] },
    httpStatus: 200,
    durationMs: 312,
    checkedAtMs,
  };
}

const done = (a: Availability, checkedAtMs = 0): AccessView => ({ state: "done", account: account(a, {}, checkedAtMs) });
const failed: AccessView = {
  state: "failed",
  error: { code: "NETWORK", category: "network", message: "Sin conexión.", detail: null, httpStatus: null },
};

describe("accessPresentation: estados de la interfaz", () => {
  it("habilitado solo con permiso confirmado", () => {
    const p = accessPresentation(done({ kind: "confirmed" }));
    expect(p.mode).toBe("enabled");
    expect(p.canRequest).toBe(true);
  });

  it("emisión no disponible → solicitar acceso", () => {
    for (const cause of ["neverApplied", "notApproved", "rejected", "cannotGoLive"] as const) {
      const p = accessPresentation(done({ kind: "notAvailable", cause }));
      expect(p.mode).toBe("request");
      expect(p.canRequest).toBe(false);
    }
    expect(REQUEST_ACCESS_TEXT).toBe(
      "Tu cuenta está conectada. Solicita acceso a LIVE proporcionando la información y las capturas que TikTok solicite.",
    );
  });

  it("solicitud pendiente según el proveedor", () => {
    const p = accessPresentation(done({ kind: "notAvailable", cause: "pending" }));
    expect(p.mode).toBe("pending");
    expect(p.label).toBe("Solicitud en revisión");
    expect(p.canRequest).toBe(false);
  });

  it("never_applied no se presenta como «nunca solicitada»: puede estar en revisión", () => {
    const p = accessPresentation(done({ kind: "notAvailable", cause: "neverApplied" }));
    expect(p.description).toMatch(/Si ya enviaste la solicitud, consúltala/);
  });

  it("estado desconocido ante fallos o datos insuficientes", () => {
    for (const v of [failed, done({ kind: "unknown", detail: "x" }), done({ kind: "contradictory", detail: "x" })]) {
      const p = accessPresentation(v);
      expect(p.mode).toBe("unknown");
      expect(p.label).not.toMatch(/no disponible|no aprobad/i);
      expect(p.canRequest).toBe(false);
    }
  });

  it("legacy y volver a entrar no ofrecen solicitar acceso", () => {
    expect(accessPresentation(done({ kind: "notAvailable", cause: "legacy" })).mode).toBe("blocked");
    expect(accessPresentation(done({ kind: "notAvailable", cause: "relog" })).mode).toBe("blocked");
  });

  it("muestra tal cual un estado de solicitud no documentado", () => {
    const v: AccessView = {
      state: "done",
      account: account({ kind: "notAvailable", cause: "notApproved" }, { applicationStatus: { state: "present", value: "on_hold" } }),
    };
    expect(accessPresentation(v).description).toBe("Estado de la solicitud según Streamlabs: «on_hold».");
  });

  it("comprobación en curso", () => {
    expect(accessPresentation({ state: "checking" }).checking).toBe(true);
  });
});

describe("shouldRefreshOnFocus: comprobar al volver sin repetir peticiones", () => {
  const t0 = 1_000_000_000;

  it("comprueba al volver si no hay permiso y pasó el intervalo", () => {
    expect(shouldRefreshOnFocus(true, done({ kind: "notAvailable", cause: "neverApplied" }, t0), 0, t0 + FOCUS_REFRESH_MIN_MS)).toBe(true);
    expect(shouldRefreshOnFocus(true, failed, 0, t0)).toBe(true);
  });

  it("no repite dentro del intervalo (ni tras una comprobación reciente)", () => {
    const v = done({ kind: "notAvailable", cause: "neverApplied" }, t0);
    expect(shouldRefreshOnFocus(true, v, 0, t0 + 5_000)).toBe(false);
    expect(shouldRefreshOnFocus(true, failed, t0, t0 + 5_000)).toBe(false);
  });

  it("no comprueba sin sesión, durante otra comprobación ni con permiso confirmado", () => {
    expect(shouldRefreshOnFocus(false, failed, 0, t0)).toBe(false);
    expect(shouldRefreshOnFocus(true, { state: "checking" }, 0, t0)).toBe(false);
    expect(shouldRefreshOnFocus(true, done({ kind: "confirmed" }), 0, t0)).toBe(false);
  });
});

describe("errorText", () => {
  it("conserva el mensaje original del proveedor", () => {
    const t = errorText({
      code: "PROVIDER_FORBIDDEN",
      category: "authorization",
      message: "Streamlabs no autorizó la solicitud (HTTP 403).",
      detail: "Unable to go live. Please try again later",
      httpStatus: 403,
    });
    expect(t).toContain("«Unable to go live. Please try again later»");
    expect(t.match(/HTTP 403/g)?.length).toBe(1);
  });
});

describe("formatos y validación", () => {
  it("tiempos", () => {
    expect(formatCountdown(61_500)).toBe("01:02");
    expect(formatElapsed(3_723_000)).toBe("01:02:03");
  });
  it("título", () => {
    expect(validateTitle("   ", 100)).not.toBeNull();
    expect(validateTitle("Hola", 100)).toBeNull();
  });
});
