// Buscador de categorías de TikTok (consulta a Streamlabs con retardo).

import { useEffect, useRef, useState } from "react";
import { api, toErrorInfo } from "../api";
import type { Category } from "../types";
import { Spinner } from "./ui";

const OTHER: Category = { id: "", name: "Otra (sin categoría)" };

export function CategoryPicker({
  value,
  onChange,
  disabled,
}: {
  value: Category | null;
  onChange: (c: Category | null) => void;
  disabled?: boolean;
}) {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<Category[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const seq = useRef(0);

  useEffect(() => {
    const q = query.trim();
    if (!q) {
      setResults([]);
      setError(null);
      return;
    }
    const id = ++seq.current;
    const timer = window.setTimeout(async () => {
      setLoading(true);
      try {
        const list = await api.searchCategories(q);
        if (id === seq.current) {
          setResults(list);
          setError(null);
        }
      } catch (err) {
        if (id === seq.current) setError(toErrorInfo(err).message);
      } finally {
        if (id === seq.current) setLoading(false);
      }
    }, 400);
    return () => window.clearTimeout(timer);
  }, [query]);

  if (value) {
    return (
      <div className="chip-row">
        <span className="chip">{value.name}</span>
        <button type="button" className="link" onClick={() => onChange(null)} disabled={disabled}>
          Cambiar
        </button>
      </div>
    );
  }

  return (
    <div className="picker">
      <div className="input-wrap">
        <input
          id="category"
          type="search"
          placeholder="Busca un juego o categoría (máx. 25 caracteres)"
          value={query}
          maxLength={60}
          onChange={(e) => setQuery(e.target.value)}
          disabled={disabled}
          autoComplete="off"
        />
        {loading && <Spinner label="Buscando" />}
      </div>
      {error && <p className="field-error">{error}</p>}
      <ul className="results" role="listbox" aria-label="Categorías">
        {results.map((c) => (
          <li key={`${c.id}-${c.name}`}>
            <button type="button" onClick={() => onChange(c)} disabled={disabled}>
              {c.name}
            </button>
          </li>
        ))}
        {results.length === 0 && (
          <li>
            <button type="button" onClick={() => onChange(OTHER)} disabled={disabled}>
              {OTHER.name}
            </button>
          </li>
        )}
      </ul>
    </div>
  );
}
