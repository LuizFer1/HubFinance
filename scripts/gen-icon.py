"""Gera os icones do HubFinance em assets/icon/ a partir do pacote de marca em assets/brand/.

Geracao offline, como freeze-inter.sh e oklch.py: o resultado e commitado e o build nunca
roda este script. Uso, da raiz do repo:

    python scripts/gen-icon.py

Requer Pillow (testado com 12.x).

- `icon.ico` e copia de `assets/brand/hubfinance.ico` (o pacote ja traz as camadas certas).
- `32/64/128/256/512x512.png` sao reducoes (LANCZOS) de `hubfinance-512.png`; `icon.icns` sai
  do mesmo 512.
- `tray-{on,error,offline,off}-{light,dark}.png` (32 px): `-light` = para barra de tarefas
  CLARA (desenho escuro), `-dark` = para barra ESCURA (desenho claro). Estado e um ponto no canto
  inferior direito com anel na cor do fundo da barra, para nao se misturar com o desenho:
  erro (#F28B82 na barra escura, #B3261E na clara), sem rede (#F2994A / #975201); "off" (hub
  parado) e a base a 45% de opacidade, sem ponto. `desktop::tray::icon_file` escolhe o nome.
"""

import shutil
from pathlib import Path

from PIL import Image, ImageDraw

ROOT = Path(__file__).resolve().parent.parent
BRAND = ROOT / "assets" / "brand"
OUT = ROOT / "assets" / "icon"

SIZES = (32, 64, 128, 256, 512)
TRAY = 32

# (cor do ponto, cor do fundo da barra para o anel) por tema da barra.
ERROR = {"light": (0xB3, 0x26, 0x1E), "dark": (0xF2, 0x8B, 0x82)}
OFFLINE = {"light": (0x97, 0x52, 0x01), "dark": (0xF2, 0x99, 0x4A)}
BAR_BG = {"light": (0xF3, 0xF3, 0xF3), "dark": (0x20, 0x20, 0x20)}

# Desenho em escala maior e reducao: o ImageDraw nao suaviza bordas.
SS = 8
DOT_R = 6  # raio do ponto, em px de 32
RING = 2
DOT_CENTER = (TRAY - 8, TRAY - 8)


def with_dot(base: Image.Image, color: tuple[int, int, int], theme: str) -> Image.Image:
    # Anel e ponto numa camada transparente em escala SS; so ela e reduzida, e entra sobre a
    # base intacta. Reamostrar a base junto borraria a logo fora do ponto.
    layer = Image.new("RGBA", (TRAY * SS, TRAY * SS), (0, 0, 0, 0))
    draw = ImageDraw.Draw(layer)
    cx, cy = (c * SS for c in DOT_CENTER)
    ring = (DOT_R + RING) * SS
    draw.ellipse((cx - ring, cy - ring, cx + ring, cy + ring), fill=(*BAR_BG[theme], 255))
    r = DOT_R * SS
    draw.ellipse((cx - r, cy - r, cx + r, cy + r), fill=(*color, 255))
    out = base.copy()
    out.alpha_composite(layer.resize((TRAY, TRAY), Image.LANCZOS))
    return out


def faded(base: Image.Image, opacity: float) -> Image.Image:
    out = base.copy()
    alpha = out.getchannel("A").point(lambda a: round(a * opacity))
    out.putalpha(alpha)
    return out


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    for old in ("1024x1024.png", "tray-attention.png", "tray-error.png"):
        (OUT / old).unlink(missing_ok=True)

    shutil.copyfile(BRAND / "hubfinance.ico", OUT / "icon.ico")
    master = Image.open(BRAND / "hubfinance-512.png").convert("RGBA")
    for size in SIZES:
        img = master if size == 512 else master.resize((size, size), Image.LANCZOS)
        img.save(OUT / f"{size}x{size}.png", optimize=True)
    master.save(OUT / "icon.icns", format="ICNS")

    for theme in ("light", "dark"):
        base = Image.open(BRAND / f"bandeja-32-para-tema-{'claro' if theme == 'light' else 'escuro'}.png").convert("RGBA")
        base.save(OUT / f"tray-on-{theme}.png", optimize=True)
        with_dot(base, ERROR[theme], theme).save(OUT / f"tray-error-{theme}.png", optimize=True)
        with_dot(base, OFFLINE[theme], theme).save(OUT / f"tray-offline-{theme}.png", optimize=True)
        faded(base, 0.45).save(OUT / f"tray-off-{theme}.png", optimize=True)


if __name__ == "__main__":
    main()
