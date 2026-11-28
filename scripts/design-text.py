"""Imprime o texto visivel de um artboard `.dc.html`, uma linha por bloco.

Serve para conferir textos do design com `git grep` em vez de "olhar o HTML".
Uso: python -I scripts/design-text.py "<pasta do design>\\Ajustes.dc.html"
"""

import html
import re
import sys


def visible_lines(source):
    # Fora o que nao e texto de tela: estilo, script e o desenho dos icones.
    for tag in ("style", "script", "svg"):
        source = re.sub(rf"<{tag}\b.*?</{tag}>", " ", source, flags=re.S | re.I)
    # Tags de bloco quebram a linha; as de texto corrido (span, b, code...) so somem.
    source = re.sub(
        r"</?(div|p|li|ul|ol|section|header|footer|main|aside|nav|h[1-6]|button|a|label|tr|td|th|table|br|dialog)\b[^>]*>",
        "\n",
        source,
        flags=re.I,
    )
    source = re.sub(r"<[^>]+>", "", source)
    for line in html.unescape(source).splitlines():
        line = " ".join(line.replace("\xa0", " ").split())
        if line:
            yield line


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit('uso: python -I scripts/design-text.py "<arquivo>.dc.html"')
    sys.stdout.reconfigure(encoding="utf-8")
    with open(sys.argv[1], encoding="utf-8") as f:
        for line in visible_lines(f.read()):
            print(line)
