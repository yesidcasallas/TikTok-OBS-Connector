import { describe, expect, it } from "vitest";
import type { AccessView, AccountInfo, Availability } from "../types";
import { accessPresentation, errorText, formatCountdown, formatElapsed, validateTitle } from "./text";

function account(availability: Availability, extra: Partial<AccountInfo["access"]> = {}): AccountInfo {
  return {
    username: "creadora",
    displayName: null,
    access: {
      reason: { state: "present", value: 0 },
      canBeLive: { state: "present", value: false },
      applicationStatus: { state: "missing" },
      userPresent: true,
      scope: "notApproved",
      availability,
      ...extra,
    },
    audience: { disabled: false, defaultValue: "0", options: [] },
    httpStatus: 200,
    durationMs: 312,
    checkedAtMs: 0,
  };
}

const done = (a: Availability): AccessView => ({ state: "done", account: account(a) });

describe("accessPresentation", () => {
  it("solo permite pedir credenciales con permiso confirmado", () => {
    expect(accessPresentation(done({ kind: "confirmed" })).canRequest).toBe(true);
    for (const v of [
      done({ kind: "notAvailable", cause: "notApproved" }),
      done({ kind: "unknown", detail: "x" }),
      done({ kind: "contradictory", detail: "x" }),
      { state: "checking" } as AccessView,
      { state: "idle" } as AccessView,
      {
        state: "failed",
        error: { code: "NETWORK", category: "network", message: "sin red", detail: null, httpStatus: null },
      } as AccessView,
    ]) {
      expect(accessPresentation(v).canRequest).toBe(false);
    }
  });

  it("un fallo o un dato ausente no se presentan como «no aprobada»", () => {
    const failed = accessPresentation({
      state: "failed",
      error: { code: "PROVIDER_UNAVAILABLE", category: "server", message: "Error 503.", detail: null, httpStatus: 503 },
    });
    expect(failed.label).not.toMatch(/no disponible|no aprobad/i);
    expect(accessPresentation(done({ kind: "unknown", detail: "x" })).label).toBe("Acceso sin confirmar");
    expect(accessPresentation(done({ kind: "contradictory", detail: "x" })).tone).toBe("warn");
  });

  it("ofrece solicitar acceso cuando corresponde", () => {
    expect(accessPresentation(done({ kind: "notAvailable", cause: "neverApplied" })).offerAccessLinks).toBe(true);
    expect(accessPresentation(done({ kind: "notAvailable", cause: "legacy" })).offerAccessLinks).toBe(false);
    expect(accessPresentation(done({ kind: "confirmed" })).offerAccessLinks).toBe(false);
  });

  it("no afirma que nunca se solicitó: puede estar en revisión", () => {
    const p = accessPresentation(done({ kind: "notAvailable", cause: "neverApplied" }));
    expect(p.description).not.toMatch(/no has solicitado/);
    expect(p.description).toMatch(/consulta su estado/);
  });

  it("muestra tal cual un estado de solicitud no documentado", () => {
    const v: AccessView = {
      state: "done",
      account: account(
        { kind: "notAvailable", cause: "notApproved" },
        { applicationStatus: { state: "present", value: "pending" } },
      ),
    };
    expect(accessPresentation(v).description).toBe("Estado de la solicitud según Streamlabs: «pending».");
  });

  it("marca la comprobación en curso", () => {
    expect(accessPresentation({ state: "checking" }).checking).toBe(true);
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
    expect(validateTitle("x".repeat(101), 100)).not.toBeNull();
  });
});
