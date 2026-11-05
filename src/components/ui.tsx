// Componentes de interfaz reutilizables.

import { useState, type ReactNode } from "react";
import type { Tone } from "../lib/text";

export function Spinner({ label }: { label?: string }) {
  return <span className="spinner" role="status" aria-label={label ?? "Cargando"} />;
}

type Variant = "primary" | "secondary" | "danger" | "ghost";

export function Button({
  children,
  onClick,
  variant = "secondary",
  disabled,
  loading,
  type = "button",
  title,
}: {
  children: ReactNode;
  onClick?: () => void;
  variant?: Variant;
  disabled?: boolean;
  loading?: boolean;
  type?: "button" | "submit";
  title?: string;
}) {
  return (
    <button
      type={type}
      className={`btn btn-${variant}`}
      onClick={onClick}
      disabled={disabled || loading}
      aria-busy={loading || undefined}
      title={title}
    >
      {loading && <Spinner />}
      <span>{children}</span>
    </button>
  );
}

/** Botón con confirmación en línea (sin diálogos nativos). */
export function ConfirmButton({
  children,
  question,
  confirmLabel,
  onConfirm,
  variant = "secondary",
  disabled,
  loading,
}: {
  children: ReactNode;
  question: ReactNode;
  confirmLabel: string;
  onConfirm: () => void;
  variant?: Variant;
  disabled?: boolean;
  loading?: boolean;
}) {
  const [asking, setAsking] = useState(false);
  if (!asking) {
    return (
      <Button variant={variant} disabled={disabled} loading={loading} onClick={() => setAsking(true)}>
        {children}
      </Button>
    );
  }
  return (
    <div className="confirm" role="group">
      <p>{question}</p>
      <div className="row">
        <Button
          variant={variant === "danger" ? "danger" : "primary"}
          onClick={() => {
            setAsking(false);
            onConfirm();
          }}
        >
          {confirmLabel}
        </Button>
        <Button variant="ghost" onClick={() => setAsking(false)}>
          Cancelar
        </Button>
      </div>
    </div>
  );
}

export function Badge({ tone, children }: { tone: Tone | "live"; children: ReactNode }) {
  return <span className={`badge badge-${tone}`}>{children}</span>;
}

export function Card({
  title,
  step,
  aside,
  children,
}: {
  title: string;
  step?: number;
  aside?: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="card">
      <header className="card-head">
        <h2>
          {step !== undefined && <span className="step">{step}</span>}
          {title}
        </h2>
        {aside}
      </header>
      {children}
    </section>
  );
}

export function Callout({ tone, title, children }: { tone: Tone; title?: string; children: ReactNode }) {
  return (
    <div className={`callout callout-${tone}`} role={tone === "bad" ? "alert" : undefined}>
      {title && <strong>{title}</strong>}
      <div>{children}</div>
    </div>
  );
}
