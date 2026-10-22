# Changelog

Todas as mudanças relevantes do HubFinance ficam aqui. O formato segue o
[Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/) e a numeração é
[semver](https://semver.org/lang/pt-BR/): a versão é a do `Cargo.toml`, e a tag é `v<versão>`.

## [Não lançado]

### Adicionado

- O dashboard respeita os estados de lançamento do app (`reportState`): uma **transferência
  interna** ou um lançamento **oculto** continua na lista, com uma etiqueta, mas não entra em
  saldo, receitas, despesas, rosca, gráfico dos últimos meses nem no saldo do dia. Os totais
  ficam iguais aos do app para o mesmo conjunto de lançamentos.
- Linha antiga (sem o campo) e valor que esta versão não conhece contam como lançamento
  normal. Nenhuma mudança no protocolo `/v1` nem no banco.
- O dashboard mostra as **metas por categoria** do app (tabela `budgets`) numa seção própria, só
  leitura: gasto do mês, meta, restante e uso, com a barra em atenção a partir de 80 % e em
  cor de despesa quando passa de 100 %. Vale a mesma regra do app: só despesa que conta nos
  totais (transferência e oculto ficam de fora), categorias fundidas resolvidas, meta de
  categoria apagada some e, com duas metas para a mesma categoria, vale a de `updatedAt` maior.
  Linha de meta fora do contrato (sem categoria, limite zero ou negativo) é ignorada e contada.

### Alterado

- **Remover um aparelho** abre um modal em dois passos com o que apagar dos dados que ele enviou,
  por área do app: **metas**, **transferências** (lançamentos, com as transferências internas
  contadas), **reservas** (com os movimentos) e **recorrências** (com os reajustes). Cada área
  mostra a contagem; "Manter tudo" é o padrão e "Apagar tudo" marca as quatro. Marcar algo leva
  a uma confirmação em vermelho com as contagens e um botão que nomeia a ação ("Remover e apagar
  150 itens"). Categorias, formas de pagamento e o perfil nunca são apagados.
- O "Desfazer" de 5 segundos cobre a remoção **e** o apagar: nada é enviado antes do prazo.
- Apagar **propaga**: o hub grava a linha como apagada (tombstone, nunca `DELETE`), com um HLC
  derivado da própria linha (sem usar o relógio do computador), e todos os celulares a recebem
  no próximo sync. Um aparelho pareado de novo também perde os dados apagados; o diálogo avisa.
  Remover e apagar acontecem na mesma transação do banco, e repetir não apaga de novo.
- Critério provisório (ainda não confirmado) para uma linha ser "do aparelho": o aparelho foi
  quem enviou a versão atual **e** quem a carimbou. Linha reenviada ou editada por outro aparelho
  não é apagada.
- Reserva só é apagada se todos os movimentos vivos dela forem do aparelho; senão ela é mantida
  (e o modal avisa) e só os movimentos do aparelho caem. Recorrência apagada leva os reajustes
  dela e não toca os lançamentos já gerados.
- Aparelhos **já removidos** que ainda têm dados (inclusive os de versões antigas) geram um
  aviso na primeira abertura, em qualquer tela, com "Decidir" e "Depois". "Depois" não grava
  nada; decidir (apagar tudo ou manter tudo) tira o aviso. Não existe lista permanente de
  aparelhos removidos.
- Banco passa para o schema v3 (coluna `devices.data_kept_at`, índice por `origin` e um id do
  próprio hub guardado em `meta`); a migração preserva aparelhos e linhas. Nenhuma mudança no
  protocolo `/v1` nem exigência de versão nova do app.

## [0.0.5] - 2026-10-07

Nenhuma mudança no protocolo `/v1` nem no banco: instalar por cima da 0.0.4 preserva `epoch`,
certificado e aparelhos pareados, e as linhas de reservas já guardadas no hub passam a
aparecer sem ressincronizar.

### Corrigido

- A página Reservas lê `reserves` e `reserveMovements` no formato que o app grava (caixinha
  `kind: "goal"`, `targetMinor`, `deadline`, `recurring { amountMinor, day, since }`, movimento
  com `amountMinor` com sinal e sem `kind`): antes toda caixinha e todo movimento ficavam fora
  do contrato e os saldos apareciam zerados.
- Paridade de regras com o app: custo essencial arredonda o total uma única vez (como
  `Math.round` do app), ritmo sugere reais inteiros para cima, meses cobertos são truncados,
  saldo negativo cobre 0 meses e o percentual da meta usa piso.
- Movimento com `amountMinor` 0 é contado como fora do contrato (o app nunca grava zero).

### Adicionado

- A reserva de emergência usa o custo essencial informado no app (`essentialOverrideMinor`)
  quando existe: a meta vira múltiplo × custo informado, e a tela mostra o total como "custo
  informado no app", sem a lista de categorias.

## [0.0.4] - 2026-10-05

Nenhuma mudança no protocolo `/v1` nem no banco: instalar por cima da 0.0.3 preserva `epoch`,
certificado e aparelhos pareados. Acompanha a fusão das categorias e formas de pagamento
padrão do app (homefinance#33).

### Corrigido

- Lançamentos que apontam para uma cópia de categoria ou forma de pagamento fundida pelo app
  (`mergedInto`) aparecem sob o padrão estável — rosca, lista, filtro de categoria e custo
  essencial da reserva de emergência — em vez de "Categoria removida", "Forma removida" ou
  "Sem categoria". Categoria apagada sem `mergedInto` continua como antes. Nenhuma mudança no
  protocolo `/v1` nem no banco.

## [0.0.3] - 2026-10-03

Nenhuma mudança no protocolo `/v1` nem no banco: instalar por cima da 0.0.2 preserva `epoch`,
certificado e aparelhos pareados.

### Adicionado

- Página **Reservas** na janela: total separado, emergência coberta (meses de custo essencial),
  guardado e retirado nos últimos 12 meses, card da reserva de emergência com meta derivada
  (múltiplo × custo essencial médio das categorias que o app indicar), caixinhas com ritmo,
  evolução de 12 meses por reserva e as últimas movimentações. Só leitura, a partir das
  tabelas `reserves` e `reserveMovements`; movimentação de reserva não é receita nem despesa
  e não altera Dashboard nem Lançamentos. O app ainda não sincroniza essas tabelas: até lá a
  página mostra o estado vazio.

### Corrigido

- O conteúdo das páginas ocupa a largura toda da janela: o teto de 1180 px deixava uma faixa
  vazia à direita com a janela maximizada.
- O rótulo de cima do eixo dos gráficos ("R$ 12 mil") não perde mais o "R": a coluna do eixo
  passou de 52 para 68 px, o que cabe até "R$ 120 mil".

## [0.0.2] - 2026-10-02

Versão para o primeiro teste de campo. Nenhuma mudança no protocolo `/v1` nem no banco:
instalar por cima da 0.0.1 preserva `epoch`, certificado e aparelhos pareados.

### Adicionado

- Log em arquivo: `<pasta de dados>/logs/hub.AAAA-MM-DD.log`, rotação diária, últimos 7
  dias; `RUST_LOG` continua valendo. Toda linha da atividade da janela vai para o arquivo.
  Nunca contém chave, hash, código de pareamento nem linha sincronizada.
- Versão visível na barra lateral ("Hub de casa · v0.0.2") e card "Sobre o hub" na tela
  Conexão com versão, pasta de dados, pasta de logs (ambas com "Copiar") e o botão "Versões
  no GitHub", que abre a página de Releases no navegador do sistema — o hub continua sem
  abrir nenhuma conexão para fora da rede local.
- `SHA256SUMS.txt` publicado junto dos instaladores; `release.yml` recusa tag diferente da
  versão do `Cargo.toml` e usa esta seção como texto da release.
- `LICENSE` (MIT) e este `CHANGELOG.md`.

### Alterado

- O rodapé do card Atividade mostra só o epoch; o caminho dos dados mudou para "Sobre o hub".
- README: seção "Updating" (como ver a versão, onde baixar, conferir o download, o que uma
  atualização preserva, não misturar formatos).
- O CI também pode ser disparado à mão (`workflow_dispatch`); foi assim que os testes rodaram
  pela primeira vez em Linux e macOS, todos verdes.

## [0.0.1] - 2026-10-01

Primeira versão publicada. Nunca testada com aparelhos reais: é a base sobre a qual o teste de
campo da 0.0.2 acontece.

### Adicionado

- Núcleo de sync na rede local: banco SQLite com uma linha JSON opaca por entidade, merge
  LWW por linha pelo HLC de `updatedAt`, `seq` monotônico como cursor e `epoch` do banco;
  rotas HTTPS `/v1/info`, `/v1/pair`, `/v1/me`, `/v1/push` e `/v1/pull` com Bearer, CORS para
  o PWA e Private Network Access.
- Pareamento por código de uso único (5 min) exibido na janela e por QR com deep link;
  revogação de aparelho; só o `sha256` da chave é guardado.
- Autoridade certificadora local gerada pelo próprio hub (restrita à LAN, `pathlen 0`),
  certificado do servidor reemitido quando o IP muda; porta HTTP 7778 com o `ca.crt` e o guia
  de instalação para Android e iPhone.
- Janela desktop (iced 0.14) com barra de título própria, tema escuro e claro (preferência em
  `ui.json`), Inter Hub e Phosphor embutidas; telas Dashboard (resumo do mês, rosca por
  categoria, barras por mês, últimos lançamentos), Lançamentos (filtros e tabela por dia) e
  Conexão (endereço, QR e código, certificado, usuários conectados com pessoa e foto, Remover
  com confirmação e Desfazer, atividade).
- Contrato de campos do dashboard: linha fora do contrato é ignorada e contada, nunca derruba
  o sync nem a janela.
- Instaladores por `cargo-packager`: Windows (`.exe` NSIS por usuário e `.msi`), Linux x64
  (`.deb` e AppImage) e macOS (`.dmg` e `.app.zip`, Apple Silicon e Intel); ícone do app;
  CI (fmt, clippy e testes em Linux, Windows e macOS) e release por tag.
- Scripts `scripts/e2e.sh` (roteiro curl de pareamento e sync) e `scripts/seed-dashboard.sh`
  (carga de exemplo do dashboard).

[Não lançado]: https://github.com/LuizFer1/HubFinance/compare/v0.0.5...HEAD
[0.0.5]: https://github.com/LuizFer1/HubFinance/compare/v0.0.4...v0.0.5
[0.0.4]: https://github.com/LuizFer1/HubFinance/compare/v0.0.3...v0.0.4
[0.0.3]: https://github.com/LuizFer1/HubFinance/compare/v0.0.2...v0.0.3
[0.0.2]: https://github.com/LuizFer1/HubFinance/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/LuizFer1/HubFinance/releases/tag/v0.0.1
