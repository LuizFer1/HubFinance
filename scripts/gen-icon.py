"""Gera o icone do HubFinance em assets/icon/ a partir do tile da marca no design.

Geracao offline, como freeze-inter.sh e oklch.py: o resultado e commitado e o build nunca
roda este script. Uso, da raiz do repo:

    python scripts/gen-icon.py

Requer Pillow (testado com 12.x).

O desenho e o tile de 32px da barra lateral (raio 8, borda de 1px no acento, glifo
house-line de 17px no acento-300) escalado: cada tamanho e desenhado de novo em vez de
reduzido de um so mestre, para a borda continuar com 1px exato no tamanho 32 e nao virar um
borrao nos tamanhos pequenos do Windows.
"""

from pathlib import Path

from PIL import Image, ImageDraw, ImageFont

ROOT = Path(__file__).resolve().parent.parent
FONT = ROOT / "assets" / "fonts" / "Phosphor.ttf"
OUT = ROOT / "assets" / "icon"

BG = (0x16, 0x18, 0x26, 255)
ACCENT = (0x91, 0x84, 0xD9, 255)
ACCENT_300 = (0xD2, 0xCE, 0xFD, 255)

# Mesmo codepoint de `HOUSE_LINE` em src/ui/icons.rs; trocar a fonte sem conferir os dois
# desenharia outro glifo sem erro nenhum.
HOUSE_LINE = "\ue2c4"

# Medidas do tile no design, em px de um tile de 32.
TILE = 32
RADIUS = 8
BORDER = 1
GLYPH = 17

# Desenho em escala maior e reducao com Lanczos: o ImageDraw nao suaviza bordas. O teto
# evita uma imagem de 8192px para o tamanho 1024.
SUPERSAMPLE = 8
MAX_CANVAS = 4096

PNG_SIZES = [32, 64, 128, 256, 512, 1024]
ICO_SIZES = [16, 24, 32, 48, 64, 128, 256]


def render(size: int) -> Image.Image:
    big = min(size * SUPERSAMPLE, max(MAX_CANVAS, size))
    unit = big / TILE
    img = Image.new("RGBA", (big, big), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    border = max(round(BORDER * unit), 1)
    draw.rounded_rectangle(
        (0, 0, big - 1, big - 1),
        radius=round(RADIUS * unit),
        fill=BG,
        outline=ACCENT,
        width=border,
    )
    font = ImageFont.truetype(str(FONT), round(GLYPH * unit))
    # Centraliza pela caixa real do glifo, nao pela linha de base: o glifo do Phosphor nao
    # ocupa a altura toda da fonte.
    left, top, right, bottom = draw.textbbox((0, 0), HOUSE_LINE, font=font)
    x = (big - (right - left)) / 2 - left
    y = (big - (bottom - top)) / 2 - top
    draw.text((x, y), HOUSE_LINE, font=font, fill=ACCENT_300)
    return img.resize((size, size), Image.Resampling.LANCZOS)


def main() -> None:
    OUT.mkdir(parents=True, exist_ok=True)
    rendered = {size: render(size) for size in sorted(set(PNG_SIZES + ICO_SIZES))}
    for size in PNG_SIZES:
        rendered[size].save(OUT / f"{size}x{size}.png", optimize=True)
    # .ico com cada tamanho desenhado no proprio tamanho (append_images), nao reduzido do
    # maior pelo Pillow.
    ico = [rendered[size] for size in ICO_SIZES]
    ico[-1].save(
        OUT / "icon.ico",
        sizes=[(s, s) for s in ICO_SIZES],
        append_images=ico[:-1],
    )
    # O Pillow monta o .icns a partir do maior e reduz os demais; acima de 32px a borda ja
    # tem mais de um pixel, entao a reducao nao perde nada visivel.
    rendered[1024].save(OUT / "icon.icns")
    for path in sorted(OUT.iterdir()):
        print(f"{path.relative_to(ROOT)}  {path.stat().st_size} bytes")


if __name__ == "__main__":
    main()
