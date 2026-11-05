// Título y categoría; obtener servidor y clave.

import { useEffect, useState } from "react";
import { api } from "../api";
import { validateTitle } from "../lib/text";
import { useApp } from "../useApp";
import type { Category } from "../types";
import { Button, Card } from "./ui";
import { CategoryPicker } from "./CategoryPicker";

export function StreamForm() {
  const { snapshot, run, pending } = useApp();
  const [title, setTitle] = useState("");
  const [category, setCategory] = useState<Category | null>(null);
  const [touched, setTouched] = useState(false);
  const [initialized, setInitialized] = useState(false);

  useEffect(() => {
    if (snapshot && !initialized) {
      setTitle(snapshot.form.title);
      setCategory(snapshot.form.category);
      setInitialized(true);
    }
  }, [snapshot, initialized]);

  if (!snapshot || snapshot.auth.status !== "signedIn") return null;
  const { form, auth } = snapshot;
  const titleError = validateTitle(title, form.titleMaxChars);
  const busy = pending.create;
  const canSubmit = !titleError && !!category && !snapshot.busy;
  const audienceType = auth.account?.audience.defaultValue ?? "0";

  return (
    <Card title="Datos del LIVE">
      <div className="field">
        <label htmlFor="title">Título</label>
        <input
          id="title"
          type="text"
          value={title}
          maxLength={form.titleMaxChars + 20}
          placeholder="Título del LIVE"
          onChange={(e) => setTitle(e.target.value)}
          onBlur={() => setTouched(true)}
          disabled={busy}
        />
        {touched && titleError && <span className="field-error">{titleError}</span>}
      </div>

      <div className="field">
        <label htmlFor="category">Categoría</label>
        <CategoryPicker value={category} onChange={setCategory} disabled={busy} />
      </div>

      <p className="notice">
        ⚠ Obtener el servidor y la clave <strong>crea una sesión LIVE</strong> en tu cuenta de TikTok. Finalízala aquí
        cuando termines.
      </p>

      <div className="row">
        <Button
          variant="primary"
          disabled={!canSubmit}
          loading={busy}
          onClick={() =>
            run(
              "create",
              () => api.createStream({ title: title.trim(), category: category!, audienceType }),
              "Servidor y clave listos",
            )
          }
        >
          Obtener servidor y clave
        </Button>
      </div>
    </Card>
  );
}
