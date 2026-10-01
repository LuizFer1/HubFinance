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
#         (extras/ttf/Inter-{Regular,Medium,SemiBold}.ttf; LICENSE.txt -> LICENSE-Inter.txt)
#
# Uso: ./scripts/freeze-inter.sh <dir-com-Inter-Regular.ttf,Inter-Medium.ttf,Inter-SemiBold.ttf>
# Requer: pip install fonttools opentype-feature-freezer
set -euo pipefail
SRC=${1:?"uso: $0 <dir das Inter-*.ttf>"}
OUT="$(dirname "$0")/../assets/fonts"
mkdir -p "$OUT"
for w in Regular Medium SemiBold; do
  pyftfeatfreeze -f tnum -R "Inter/Inter Hub" "$SRC/Inter-$w.ttf" "$OUT/InterHub-$w.ttf"
done
