// Estado global: instantánea del backend, avisos y acciones sin duplicados.

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";
import { api, onState, toErrorInfo } from "./api";
import { errorText, shouldRefreshOnFocus } from "./lib/text";
import type { Snapshot } from "./types";

export type ToastKind = "success" | "error" | "info";
export interface Toast {
  id: number;
  kind: ToastKind;
  title: string;
  message?: string;
}

interface AppContextValue {
  snapshot: Snapshot | null;
  toasts: Toast[];
  notify: (kind: ToastKind, title: string, message?: string) => void;
  dismiss: (id: number) => void;
  /** Ejecuta una acción evitando duplicados y mostrando el resultado. */
  run: <T>(key: string, fn: () => Promise<T>, success?: string) => Promise<T | undefined>;
  pending: Record<string, boolean>;
}

const AppContext = createContext<AppContextValue | null>(null);

function isSnapshot(v: unknown): v is Snapshot {
  return !!v && typeof v === "object" && "auth" in v && "form" in v;
}

export function AppProvider({ children }: { children: ReactNode }) {
  const [snapshot, setSnapshot] = useState<Snapshot | null>(null);
  const [toasts, setToasts] = useState<Toast[]>([]);
  const [pending, setPending] = useState<Record<string, boolean>>({});
  const pendingRef = useRef<Record<string, boolean>>({});
  const nextId = useRef(1);

  const dismiss = useCallback((id: number) => {
    setToasts((t) => t.filter((x) => x.id !== id));
  }, []);

  const notify = useCallback(
    (kind: ToastKind, title: string, message?: string) => {
      const id = nextId.current++;
      setToasts((t) => [...t.slice(-2), { id, kind, title, message }]);
      window.setTimeout(() => dismiss(id), kind === "error" ? 9000 : 4000);
    },
    [dismiss],
  );

  useEffect(() => {
    let alive = true;
    let unlisten: (() => void) | undefined;
    api.getState().then((s) => alive && setSnapshot(s)).catch(() => {});
    onState((s) => setSnapshot(s)).then((u) => (unlisten = u));
    return () => {
      alive = false;
      unlisten?.();
    };
  }, []);

  const run = useCallback(
    async <T,>(key: string, fn: () => Promise<T>, success?: string) => {
      if (pendingRef.current[key]) return undefined;
      pendingRef.current = { ...pendingRef.current, [key]: true };
      setPending(pendingRef.current);
      try {
        const result = await fn();
        if (isSnapshot(result)) setSnapshot(result);
        if (success) notify("success", success);
        return result;
      } catch (err) {
        const e = toErrorInfo(err);
        if (e.code === "AUTH_CANCELLED") notify("info", e.message);
        else notify("error", "No se pudo completar la acción", errorText(e));
        api.getState().then(setSnapshot).catch(() => {});
        return undefined;
      } finally {
        const next = { ...pendingRef.current };
        delete next[key];
        pendingRef.current = next;
        setPending(next);
      }
    },
    [notify],
  );

  // Al volver a la aplicación (p. ej., tras enviar la solicitud en TikTok),
  // comprueba el permiso sin repetir peticiones: como mucho una vez cada 30 s,
  // nunca durante otra comprobación y nunca si ya está confirmado.
  const lastFocusCheck = useRef(0);
  useEffect(() => {
    const onFocus = () => {
      if (document.visibilityState === "hidden" || !snapshot) return;
      const now = Date.now();
      if (!shouldRefreshOnFocus(snapshot.auth.status === "signedIn", snapshot.auth.access, lastFocusCheck.current, now)) {
        return;
      }
      lastFocusCheck.current = now;
      api.refreshAccount().then(setSnapshot).catch(() => {
        // El error queda reflejado en el estado (comprobación fallida).
        api.getState().then(setSnapshot).catch(() => {});
      });
    };
    window.addEventListener("focus", onFocus);
    document.addEventListener("visibilitychange", onFocus);
    return () => {
      window.removeEventListener("focus", onFocus);
      document.removeEventListener("visibilitychange", onFocus);
    };
  }, [snapshot]);

  const value = useMemo(
    () => ({ snapshot, toasts, notify, dismiss, run, pending }),
    [snapshot, toasts, notify, dismiss, run, pending],
  );
  return <AppContext.Provider value={value}>{children}</AppContext.Provider>;
}

export function useApp(): AppContextValue {
  const ctx = useContext(AppContext);
  if (!ctx) throw new Error("useApp fuera de AppProvider");
  return ctx;
}

/** Fuerza un re-render periódico (cuenta atrás y tiempo del LIVE). */
export function useNow(intervalMs = 1000): number {
  const [now, setNow] = useState(() => Date.now());
  useEffect(() => {
    const t = window.setInterval(() => setNow(Date.now()), intervalMs);
    return () => window.clearInterval(t);
  }, [intervalMs]);
  return now;
}
