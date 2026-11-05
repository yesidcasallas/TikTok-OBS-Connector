import { describe, expect, it } from "vitest";
import { accessInfo, formatCountdown, formatElapsed, validateTitle } from "./text";

describe("accessInfo", () => {
  it("distingue aprobado, sin acceso y desconocido", () => {
    expect(accessInfo("approved", true).tone).toBe("ok");
    expect(accessInfo("notApproved", false).tone).toBe("bad");
    expect(accessInfo("rejected", null).tone).toBe("bad");
    expect(accessInfo("unknown", null).tone).toBe("neutral");
    expect(accessInfo("unknown", false).tone).toBe("bad");
  });
});

describe("formatos de tiempo", () => {
  it("cuenta atrás mm:ss", () => {
    expect(formatCountdown(300_000)).toBe("05:00");
    expect(formatCountdown(61_500)).toBe("01:02");
    expect(formatCountdown(-1)).toBe("00:00");
  });
  it("transcurrido hh:mm:ss", () => {
    expect(formatElapsed(3_723_000)).toBe("01:02:03");
    expect(formatElapsed(Number.NaN)).toBe("00:00:00");
  });
});

describe("validateTitle", () => {
  it("exige texto y respeta el máximo", () => {
    expect(validateTitle("   ", 100)).not.toBeNull();
    expect(validateTitle("Hola", 100)).toBeNull();
    expect(validateTitle("x".repeat(101), 100)).not.toBeNull();
    expect(validateTitle("ñ".repeat(100), 100)).toBeNull();
    expect(validateTitle("a\u0007b", 100)).not.toBeNull();
  });
});
