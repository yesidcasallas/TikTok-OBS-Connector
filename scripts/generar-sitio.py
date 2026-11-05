#!/usr/bin/env python3
"""Genera site/privacidad.html y site/terminos.html desde los borradores en
docs/integracion-tiktok/. Solo biblioteca estándar (subconjunto de Markdown:
títulos, párrafos, listas, tablas, citas, negrita, código y enlaces).

Uso: python3 scripts/generar-sitio.py
"""
import html
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
DOCS = ROOT / "docs" / "integracion-tiktok"
SITE = ROOT / "site"

TEMPLATE = """<!doctype html>
<html lang="es">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width, initial-scale=1">
<title>{title} · Virelo</title>
<link rel="icon" href="icono.png">
<link rel="stylesheet" href="estilo.css">
</head>
<body>
<header class="top">
  <a class="brand" href="index.html"><img src="icono.png" alt="" width="32" height="32"> Virelo</a>
  <nav><a href="privacidad.html">Privacidad</a> <a href="terminos.html">Términos</a></nav>
</header>
<main class="doc">
{body}
</main>
<footer>Virelo no está afiliado a TikTok, OBS Project ni Streamlabs. · <a href="privacidad.html">Privacidad</a> · <a href="terminos.html">Términos</a></footer>
</body>
</html>
"""


def inline(text: str) -> str:
    text = html.escape(text, quote=False)
    text = re.sub(r"`([^`]+)`", r"<code>\1</code>", text)
    text = re.sub(r"\*\*([^*]+)\*\*", r"<strong>\1</strong>", text)

    def link(m: re.Match) -> str:
        label, url = m.group(1), m.group(2)
        if url.endswith(".md"):  # enlaces entre borradores → páginas del sitio
            url = {"politica-privacidad.md": "privacidad.html", "terminos.md": "terminos.html"}.get(url, url)
        return f'<a href="{html.escape(url)}">{label}</a>'

    return re.sub(r"\[([^\]]+)\]\(([^)]+)\)", link, text)


def convert(md: str) -> str:
    out, para, lines = [], [], md.splitlines()
    i = 0

    def flush():
        if para:
            out.append("<p>" + inline(" ".join(para)) + "</p>")
            para.clear()

    while i < len(lines):
        line = lines[i].rstrip()
        if not line.strip():
            flush()
            i += 1
            continue
        if m := re.match(r"^(#{1,4})\s+(.*)$", line):
            flush()
            level = len(m.group(1))
            out.append(f"<h{level}>{inline(m.group(2))}</h{level}>")
        elif line.startswith(">"):
            flush()
            quote = []
            while i < len(lines) and lines[i].startswith(">"):
                quote.append(lines[i].lstrip("> ").rstrip())
                i += 1
            out.append('<blockquote>' + inline(" ".join(quote)) + "</blockquote>")
            continue
        elif line.startswith("|"):
            flush()
            rows = []
            while i < len(lines) and lines[i].startswith("|"):
                cells = [c.strip() for c in lines[i].strip().strip("|").split("|")]
                if not all(re.fullmatch(r":?-{3,}:?", c) for c in cells):
                    rows.append(cells)
                i += 1
            head, *body = rows
            t = "<table><thead><tr>" + "".join(f"<th>{inline(c)}</th>" for c in head) + "</tr></thead><tbody>"
            t += "".join("<tr>" + "".join(f"<td>{inline(c)}</td>" for c in r) + "</tr>" for r in body)
            out.append(t + "</tbody></table>")
            continue
        elif re.match(r"^(- |\d+\. )", line):
            flush()
            ordered = bool(re.match(r"^\d+\. ", line))
            tag = "ol" if ordered else "ul"
            items = []
            while i < len(lines) and re.match(r"^(- |\d+\. )", lines[i]):
                items.append(re.sub(r"^(- |\d+\. )", "", lines[i]).rstrip())
                i += 1
            out.append(f"<{tag}>" + "".join(f"<li>{inline(x)}</li>" for x in items) + f"</{tag}>")
            continue
        else:
            para.append(line.strip())
        i += 1
    flush()
    return "\n".join(out)


def main() -> None:
    for src, dst, title in [
        ("politica-privacidad.md", "privacidad.html", "Política de privacidad"),
        ("terminos.md", "terminos.html", "Términos de uso"),
    ]:
        body = convert((DOCS / src).read_text(encoding="utf-8"))
        (SITE / dst).write_text(TEMPLATE.format(title=title, body=body), encoding="utf-8")
        print(f"site/{dst} generado desde docs/integracion-tiktok/{src}")


if __name__ == "__main__":
    main()
