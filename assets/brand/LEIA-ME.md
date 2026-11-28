# HomeFinance · HubFinance — marca

## A ideia: sob o mesmo teto
Um telhado protegendo um ponto dourado. O ponto é o que é da família: as contas, as caixinhas, os dados. Eles ficam debaixo do mesmo teto, nos aparelhos da casa.
- **HomeFinance** (celular): o telhado e o ponto.
- **HubFinance** (computador): o ponto emite um sinal, e o sinal não passa do telhado. É a central que sincroniza a casa pelo Wi-Fi sem sair dela.

Sem cifrão, moeda, gráfico, cofre ou gradiente. Duas cores: o telhado em tom claro (ou grafite) e o ponto em dourado.

## Cores
| Uso | Cor |
|---|---|
| Grafite: fundo do ícone e texto em fundo claro | #131313 |
| Dourado: o ponto, em fundo escuro | #E2C15A |
| Telhado e texto em fundo escuro | #F2F2F2 |
| Dourado para fundo claro (o ponto, sem o quadrado) | #876C00 |

O dourado #E2C15A tem pouco contraste no branco. Em fundo claro, use o ícone com quadrado grafite (`*-simbolo-app.svg`), a versão `*-fundo-claro` (ponto em #876C00) ou a mono escura.

## Nome escrito
Manrope (SIL Open Font License): "Home"/"Hub" em Bold 700 e "Finance" em Regular 400, espaçamento −1,5%. Nos SVG o texto já está convertido em curvas.

## Arquivos
- `1-simbolo/`
  - `*-simbolo-app.svg`: ícone colorido (quadrado grafite, telhado claro e ponto dourado). Serve em qualquer fundo.
  - `*-simbolo-fundo-escuro.svg` / `*-simbolo-fundo-claro.svg`: só o desenho, em duas cores, para cada tipo de fundo.
  - `*-simbolo-mono-escura.svg` / `*-mono-clara.svg`: uma cor só (grafite para fundo claro, branco para fundo escuro).
  - `*-simbolo-maskable.svg`: fundo cheio até a borda e desenho dentro do círculo central de 80%.
  - `*-pequeno-*.svg`: desenhado pixel a pixel na grade de 16. Use em 16 px; de 24 px para cima, use o símbolo vetorial.
- `2-assinatura/`: símbolo e nome, horizontal e empilhada. Cada uma tem quatro versões: colorida para fundo claro, colorida para fundo escuro, mono escura e mono clara.
- `3-exportados/`: já gerados a partir dos SVG.
  - `app-celular/`:
    - favicon.svg e favicon-16/32.png;
    - icon-192/512.png e icon-maskable-192/512.png;
    - apple-touch-icon.png (180 px, opaco).
  - `hub-windows/`:
    - hubfinance.ico (16, 24, 32, 48, 64 e 256);
    - ícones da bandeja em 16 e 32 px para barra clara e escura (PNG e SVG);
    - hubfinance-512.png.
  - `landing/`: assinatura horizontal para fundo claro e escuro, og-image-1200x630 (PNG e SVG).

## Regras rápidas
- Área de respiro: 1/4 da altura do símbolo em volta da assinatura.
- Tamanho mínimo:
  - símbolo vetorial: 24 px;
  - abaixo disso, a versão pequena;
  - assinatura horizontal: 120 px de largura.
- Não aplicar sombra, contorno nem gradiente. Não trocar as cores do telhado e do ponto entre si.
