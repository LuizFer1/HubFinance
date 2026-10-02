#!/usr/bin/env python3
"""Converte as cores `oklch(L C h)` do handoff para `#rrggbb` e imprime as tabelas da paleta.

Documentacao executavel: o Rust (`src/dashboard/colors.rs`) recebe os literais que este script
imprime, e um teste os fixa. Nada e convertido em runtime: o iced so conhece sRGB, e uma
conversao em runtime seria mais um lugar para o tom divergir entre os dois temas.

Conversao: OKLCH -> OKLab -> LMS (cubo) -> sRGB linear -> gamma sRGB, com as matrizes de
Bjorn Ottosson (https://bottosson.github.io/posts/oklab/). Fora do gamut, cada canal e
cortado em [0, 1] (clamp), que e o que o navegador do prototipo faz na pratica.

Uso: python scripts/oklch.py
"""

import math

# Token do app -> (escuro, claro). `slate` ja vem em hex no handoff ("cinza").
PALETTE = [
    ("slate", "#b2b6ca", "#75798c"),
    ("rose", (0.76, 0.11, 10), (0.6, 0.14, 10)),
    ("red", (0.76, 0.11, 32), (0.6, 0.14, 32)),
    ("orange", (0.77, 0.11, 58), (0.62, 0.14, 58)),
    ("amber", (0.8, 0.11, 85), (0.64, 0.13, 85)),
    ("lime", (0.8, 0.11, 125), (0.62, 0.13, 125)),
    ("emerald", (0.78, 0.11, 155), (0.6, 0.12, 155)),
    ("teal", (0.78, 0.1, 185), (0.6, 0.1, 185)),
    ("sky", (0.77, 0.1, 230), (0.6, 0.11, 230)),
    ("indigo", (0.74, 0.11, 265), (0.56, 0.13, 265)),
    ("violet", (0.74, 0.12, 295), (0.56, 0.14, 295)),
    ("fuchsia", (0.76, 0.12, 335), (0.58, 0.15, 335)),
]

# Receita e despesa: base (barra) e texto, por modo.
MONEY = [
    ("INCOME", (0.8, 0.12, 158), (0.6, 0.12, 158)),
    ("INCOME_FG", (0.82, 0.12, 158), (0.5, 0.11, 158)),
    ("EXPENSE", (0.74, 0.13, 22), (0.6, 0.15, 22)),
    ("EXPENSE_FG", (0.78, 0.12, 22), (0.52, 0.15, 22)),
]


def oklch_to_hex(lightness, chroma, hue):
    a = chroma * math.cos(math.radians(hue))
    b = chroma * math.sin(math.radians(hue))
    l_ = lightness + 0.3963377774 * a + 0.2158037573 * b
    m_ = lightness - 0.1055613458 * a - 0.0638541728 * b
    s_ = lightness - 0.0894841775 * a - 1.2914855480 * b
    l, m, s = l_**3, m_**3, s_**3
    r = 4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s
    g = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s
    bl = -0.0041960863 * l - 0.7034186147 * m + 1.7076147010 * s
    return "#" + "".join(f"{gamma(c):02x}" for c in (r, g, bl))


def gamma(linear):
    c = min(1.0, max(0.0, linear))
    v = 12.92 * c if c <= 0.0031308 else 1.055 * c ** (1 / 2.4) - 0.055
    return round(v * 255)


def show(value):
    return value if isinstance(value, str) else oklch_to_hex(*value)


if __name__ == "__main__":
    print("token      escuro   claro")
    for token, dark, light in PALETTE:
        print(f"{token:<10} {show(dark)}  {show(light)}")
    print()
    for name, dark, light in MONEY:
        print(f"{name:<10} {show(dark)}  {show(light)}")
