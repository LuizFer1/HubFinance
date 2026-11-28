"""Extrai os icones de traco (viewBox 0 0 24 24) dos artboards `.dc.html` do design.

Geracao offline: o resultado (`assets/icons/*.svg`) e commitado e o build nao roda isto.
Uso: python -I scripts/extract-icons.py "<pasta do design>"

Cada forma unica vira um arquivo. A mesma forma com espessura de traco diferente (o check de
4 px do seletor, o de 2.6 px da lista) vira outro arquivo, `<nome>-w<espessura>`, porque o
GPUI nao muda a espessura de um SVG na hora de desenhar. Linecap/linejoin sao sempre `round`
(o design usa so isso; os poucos sem o atributo eram o padrao `butt` por omissao e nao se
distinguem a 18 px). Tamanho e cor saem do arquivo: a cor e `currentColor` e quem desenha
escolhe. Forma sem nome na tabela: o script avisa (com o texto em volta) e nao grava.
"""

import collections
import pathlib
import re
import sys

# Nome por forma: o inicio do corpo do SVG (sem espacos nas pontas) -> nome do arquivo.
NAMES = {
    '<circle cx="12" cy="12" r="3"></circle><circle cx="12" cy="3.5"': "hub",
    '<rect x="3" y="3" width="7" height="9"': "dashboard",
    '<path d="M9 6h12M9 12h12M9 18h12': "list",
    '<path d="M12 21s7.5-3.6 7.5-9.5V5.2L12 2.5 4.5 5.2v6.3C4.5 17.4 12 21 12 21z"></path><path d="m9 12': "shield-check",
    '<path d="M12 21s7.5-3.6 7.5-9.5V5.2L12 2.5 4.5 5.2v6.3C4.5 17.4 12 21 12 21z"></path>': "shield",
    '<path d="M9 18h6M10 21.5h4': "bulb",
    '<path d="M5 12.5a10 10 0 0 1 14 0': "wifi",
    '<circle cx="12" cy="12" r="3"></circle><path d="M19.4 15a1.7': "settings",
    '<path d="M20 6 9 17l-5-5"></path>': "check",
    '<path d="M21 12a9 9 0 1 1-2.6-6.4L21 8"></path>': "sync",
    '<path d="M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9z"></path>': "moon",
    '<circle cx="12" cy="12" r="4"></circle><path d="M12 2v2': "sun",
    '<circle cx="13.5" cy="6.5" r="1"></circle>': "palette",
    '<path d="M9.9 9.9a3 3 0 1 0 4.2 4.2': "eye-off",
    '<path d="M12 2v10M18.4 6.6': "power",
    '<circle cx="12" cy="12" r="9"></circle><path d="M12 11v5M12 8h.01"></path>': "info",
    '<path d="M12 3l1.9 5.1L19 10': "sparkle",
    '<path d="M2 12s3.5-7 10-7 10 7 10 7-3.5 7-10 7S2 12 2 12z"></path><circle': "eye",
    '<path d="M15 3h6v6M10 14 21 3': "external-link",
    '<rect x="9" y="9" width="12" height="12" rx="2"></rect>': "copy",
    '<path d="M3 7a2 2 0 0 1 2-2h4l2 2h8': "folder",
    '<rect x="4" y="11" width="16" height="10" rx="2"></rect>': "lock",
    '<path d="M6 6l12 12M18 6 6 18"></path>': "close",
    '<path d="m9 18 6-6-6-6"></path>': "chevron-right",
    '<path d="m15 18-6-6 6-6"></path>': "chevron-left",
    '<path d="m6 9 6 6 6-6"></path>': "chevron-down",
    '<circle cx="12" cy="12" r="9" fill="none" stroke="var(--warning)"': "half-circle-warning",
    '<path d="M3 6h18M8 6V4h8v2': "trash",
    '<path d="M12 5v14M5 12h14"></path>': "plus",
    '<path d="m17 2 4 4-4 4"></path>': "repeat",
    '<path d="M17 7 7 17M16 17H7V8"></path>': "arrow-down-left",
    '<path d="M7 17 17 7M8 7h9v9"></path>': "arrow-up-right",
    '<rect x="3" y="4" width="18" height="17" rx="2"></rect>': "calendar",
    '<rect x="7" y="2" width="10" height="20" rx="2"></rect>': "phone",
    '<circle cx="12" cy="13" r="8"></circle><path d="M12 9v4l2 2M9 2h6"></path>': "timer",
    '<circle cx="12" cy="12" r="9"></circle><path d="M3 12h18M12 3a14 14': "globe",
    '<circle cx="9" cy="8" r="4"></circle>': "people",
    '<path d="M22 12h-4l-3 8L9 4l-3 8H2"></path>': "pulse",
    '<rect x="3" y="7" width="18" height="13" rx="2"></rect><path d="M16 7V5': "briefcase",
    '<path d="M3 3v18h18M8 17v-5': "chart-bars",
    '<path d="M21 15.9A10 10 0 1 1 8 2.8"></path>': "pie",
    '<path d="M5 3h14v18l-3-2-2 2-2-2-2 2-2-2-3 2z"></path>': "receipt",
    '<path d="M3 4h2l2.4 11.2': "cart",
    '<path d="M20.8 4.6a5.5 5.5 0 0 0-7.8 0': "heart",
    '<path d="M5 16h14M3 12l2.5-6h13l2.5 6v5H3z"></path>': "car",
    '<path d="M3 5h18l-7 8v6l-4-2v-4z"></path>': "filter",
    '<rect x="3" y="7" width="18" height="13" rx="2"></rect><path d="M3 11h18M8 3h8l2 4H6z"></path>': "box",
    '<path d="M7 4v16M3 8l4-4 4 4M17 20V4M13 16l4 4 4-4"></path>': "arrows-updown",
    '<path d="M3 17l6-6 4 4 8-8M15 7h6v6"></path>': "trend-up",
    '<rect x="3" y="3" width="7" height="7" rx="1"></rect>': "qr-code",
    '<path d="M12 4v16M5 8h14M5 8l-3 6': "scale",
    '<path d="M10.3 3.9 1.8 18': "warning",
    '<path d="M17.8 19.2 16 11l3.5-3.5': "plane",
    '<rect x="3" y="5" width="18" height="15" rx="2"></rect><path d="M3 10h18M8 15h3"></path>': "credit-card",
    '<rect x="3" y="8" width="18" height="13" rx="1.5"></rect>': "gift",
    '<circle cx="12" cy="12" r="9"></circle><path d="M12 7v6M12 16.5h.01"></path>': "alert-circle",
}

SVG = re.compile(r'<svg\b[^>]*viewBox="0 0 24 24"[^>]*>.*?</svg>', re.S)


def parse(svg):
    m = re.match(r"<svg\b([^>]*)>(.*)</svg>", svg, re.S)
    attrs = dict(re.findall(r'([\w-]+)="([^"]*)"', m.group(1)))
    body = m.group(2).strip()
    return attrs.get("stroke-width", "2"), body


def name_of(body):
    # Prefixos mais longos primeiro: "shield" e prefixo de "shield-check".
    for prefix in sorted(NAMES, key=len, reverse=True):
        if body.startswith(prefix):
            return NAMES[prefix]
    return None


def text_around(text, m):
    """Texto visivel em volta do SVG, para dar nome sem abrir o navegador."""
    before = re.sub(r"<[^>]+>", " ", text[max(0, m.start() - 200) : m.start()])
    after = re.sub(r"<[^>]+>", " ", text[m.end() : m.end() + 200])
    return (" ".join(before.split())[-30:] + " >< " + " ".join(after.split())[:30]).strip()


def main(folder, out):
    shapes = collections.OrderedDict()  # (nome, espessura) -> corpo
    unnamed = {}
    for path in sorted(pathlib.Path(folder).glob("*.dc.html")):
        text = path.read_text(encoding="utf-8")
        for m in SVG.finditer(text):
            width, body = parse(m.group(0))
            name = name_of(body)
            if name is None:
                unnamed.setdefault(body, (path.name, text_around(text, m)))
                continue
            shapes.setdefault((name, width), body)
    for n, (body, (src, ctx)) in enumerate(unnamed.items(), 1):
        print(f"AVISO sem nome (icon-{n:02}): {src}: {ctx!r}: {body[:60]}", file=sys.stderr)
    # A espessura que o design usa primeiro leva o nome puro; as outras, o sufixo -w<espessura>.
    default = {}
    for name, width in shapes:
        default.setdefault(name, width)
    out = pathlib.Path(out)
    out.mkdir(parents=True, exist_ok=True)
    written = set()
    for (name, width), body in shapes.items():
        file = name if width == default[name] else f"{name}-w{width.replace('.', '_')}"
        body = re.sub(r'(stroke|fill)="var\(--[\w-]+\)"', r'\1="currentColor"', body)
        svg = (
            '<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" fill="none" '
            f'stroke="currentColor" stroke-width="{width}" stroke-linecap="round" '
            f'stroke-linejoin="round">{body}</svg>\n'
        )
        (out / f"{file}.svg").write_text(svg, encoding="utf-8", newline="\n")
        written.add(f"{file}.svg")
    for old in out.glob("*.svg"):
        if old.name not in written:
            old.unlink()
    print(f"{len(written)} icones em {out}")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit('uso: python -I scripts/extract-icons.py "<pasta do design>"')
    root = pathlib.Path(__file__).resolve().parent.parent
    main(sys.argv[1], root / "assets" / "icons")
