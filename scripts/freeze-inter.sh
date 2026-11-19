#!/usr/bin/env bash
# Gera assets/fonts/InterHub-*.ttf a partir da Inter 4.1 estatica, congelando a feature
# OpenType `tnum` (digitos tabulares) e renomeando a familia para "Inter Hub".
#
# Por que: o iced 0.14 nao expoe features OpenType, e o design exige digitos tabulares em todo
# numero. Congelar a feature na fonte e o unico jeito. O rename evita que uma Inter instalada no
# sistema seja escolhida no lugar desta. OFL 1.1 permite modificar e redistribuir; a Inter nao
# declara Reserved Font Name.
#
# Origem: https://github.com/rsms/inter/releases/download/v4.1/Inter-4.1.zip
#         (extras/ttf/Inter-{Light,Regular,Medium,SemiBold}.ttf; LICENSE.txt -> LICENSE-Inter.txt)
#
# Uso: ./scripts/freeze-inter.sh <dir-com-Inter-Light.ttf,Inter-Regular.ttf,Inter-Medium.ttf,Inter-SemiBold.ttf>
# Requer: pip install fonttools opentype-feature-freezer
#
# Rodar de novo regrava os quatro arquivos com bytes diferentes (so muda `head.modified`); para
# acrescentar um peso, commite so o arquivo novo. Na Light o nome de familia legado (id 1) fica
# "Inter Hub Light" e o tipografico (id 16) "Inter Hub": o fontdb do iced le o 16 primeiro, entao
# ela entra na mesma familia com peso 300 (teste `fonts::tests`).
set -euo pipefail
SRC=${1:?"uso: $0 <dir das Inter-*.ttf>"}
OUT="$(dirname "$0")/../assets/fonts"
mkdir -p "$OUT"
for w in Light Regular Medium SemiBold; do
  pyftfeatfreeze -f tnum -R "Inter/Inter Hub" "$SRC/Inter-$w.ttf" "$OUT/InterHub-$w.ttf"
done
