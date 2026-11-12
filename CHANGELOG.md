# Changelog

Todas as mudanças relevantes do HubFinance ficam aqui. O formato segue o
[Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/) e a numeração é
[semver](https://semver.org/lang/pt-BR/): a versão é a do `Cargo.toml`, e a tag é `v<versão>`.

## [Não lançado]

Nenhuma mudança no protocolo `/v1` nem no banco: o hub continua guardando JSON opaco.

### Adicionado

- **Conta pessoal e comercial no dashboard.** O app passa a marcar cada linha com `book`
  (`personal` ou `business`; ausente, nulo ou desconhecido vale `personal`) e a sincronizar as
  tabelas `books`, `assets` e `debts`. O hub lê o campo e calcula cada tela sobre as linhas de
  **uma** conta: os totais pessoais não incluem mais lançamentos do comércio, nem o contrário.
  Categoria ou forma de pagamento de outra conta aparece como "Sem categoria" / "Sem forma de
  pagamento". O perfil (`users`) vale nas duas contas.
- **Seletor Pessoal | Comercial** no cabeçalho do dashboard, de lançamentos e de reservas. Só
  aparece quando o app criou a conta comercial (uma linha `books` viva do tipo `business`);
  abre sempre em Pessoal e a escolha não é gravada.
- **Bloco "Negócio" no dashboard da conta comercial** (somente leitura), acima do resumo: receitas,
  despesas e resultado do mês selecionado; investimentos, capital em estoque, capital em
  equipamento e capital total aplicado (a soma dos três); e dívidas totais (saldo devedor das
  dívidas vivas). Bens e dívidas nunca entram em receita nem em despesa, linhas apagadas ficam
  fora das somas e a conta pessoal não mostra o bloco. O hub lê `assets` e `debts` e conta como
  "linhas ignoradas" as que fugirem do contrato (tipo de bem desconhecido, saldo que não é
  inteiro).

### Atualizar nesta ordem

Atualize o hub **antes** de criar a conta comercial no app. Um hub antigo ignora `book` e soma
tudo junto; um app antigo não envia `book`, e o hub novo trata tudo como pessoal (o comportamento
de hoje).

## [1.1.0] - 2026-11-07

Nenhuma mudança no protocolo `/v1` nem no banco. Ícone na bandeja, início com o Windows e
instância única são só do Windows; o polimento das telas vale para os três sistemas.

### Adicionado

- **Ícone ao lado do relógio** (Windows). Fechar a janela agora só fecha a janela: o hub
  continua ligado e sincronizando. Clique no ícone abre a janela de volta; o botão direito tem
  "Abrir o HubFinance" e "Desligar o hub" — que é o único jeito de encerrar o programa. O tooltip
  mostra o estado ("ligado · 2 aparelhos", "sem rede", "com erro").
- **Iniciar com o Windows**, no card "Sobre o hub" da tela Conexão. Ligado por padrão na
  primeira abertura desta versão; grava um valor por usuário em
  `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` (sem administrador) e o regrava a cada
  abertura, para seguir o programa depois de uma atualização. Aberto pelo Windows, o hub sobe só
  com o ícone, sem janela; se a opção estiver desligada e ainda assim for aberto assim (valor
  esquecido por uma desinstalação), sai na hora.
- **Uma instância só.** Abrir o HubFinance com ele já aberto traz a janela em vez de abrir um
  segundo programa.
- Se o hub falhar ao subir sem janela (porta ocupada, por exemplo), a janela abre sozinha com o
  erro.

### Alterado

- O `ui.json` pode ter `"autostart": false` quando a pessoa desliga a opção; arquivos antigos
  continuam válidos.
- Atualizar: "feche o hub antes de instalar" agora é "Desligar o hub" no ícone ao lado do relógio.
- Rótulos em frase normal em vez de caixa alta ("Saldo do mês", "Endereço do hub"), também no
  cabeçalho da tabela de lançamentos e na barra lateral.
- Tela Conexão: o endereço do hub fica num campo da largura do card, acima do QR, e aparece
  inteiro mesmo com a janela na largura mínima. As pastas em "Sobre o hub" quebram só entre
  pastas, cada uma na sua linha. O vazio de "Usuários conectados" usa o ícone de celular.
- Dashboard: o total no centro da rosca diminui a fonte quando precisa, para não encostar no
  anel; o nome da categoria mostrado ao passar o mouse corta em 14 caracteres.
- Reservas: sem meta, o card da emergência diz "Sem meta" uma vez só, e "Emergência coberta"
  mostra "Sem meta" no lugar de um traço. Retirada zerada aparece como "R$ 0,00" em cinza, sem
  sinal nem a nota repetida. Os cards de resumo ficam na mesma altura.
- Legendas e frases curtas sem ponto final.

### Corrigido

- Trocar o tema duas vezes seguidas (ou o tema logo depois de outra preferência) podia deixar no
  `ui.json` a escolha anterior: as gravações agora respeitam a ordem dos cliques.
- O subtítulo dos lançamentos não diz mais "sincronizados de 0 aparelhos", e "1 lançamento"
  fica no singular.

### Conhecido

- Desinstalar não remove o valor `Run`; no próximo login o Windows ignora o programa ausente.
- Desabilitar a entrada em "Aplicativos de inicialização" do Gerenciador de Tarefas não muda a
  opção no card (o Windows só a marca como desabilitada).

## [1.0.0] - 2026-10-21

Nenhuma mudança no protocolo `/v1`. O banco sobe para o schema v3 na primeira abertura (a migração preserva aparelhos e linhas).

**Provisório:** o critério de quando uma linha é "do aparelho" (`store/erase.rs::owned_by`) e a inclusão de **recorrências** (com os reajustes) entre as áreas apagáveis ainda não foram confirmados pelo dono do produto e entram nesta versão como provisórios; podem mudar numa versão seguinte.

A 1.0.0 marca o primeiro uso diário do hub, por decisão do dono do produto; não indica que o provisório acima foi confirmado.

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

[Não lançado]: https://github.com/LuizFer1/HubFinance/compare/v1.1.0...HEAD
[1.1.0]: https://github.com/LuizFer1/HubFinance/compare/v1.0.0...v1.1.0
[1.0.0]: https://github.com/LuizFer1/HubFinance/compare/v0.0.5...v1.0.0
[0.0.5]: https://github.com/LuizFer1/HubFinance/compare/v0.0.4...v0.0.5
[0.0.4]: https://github.com/LuizFer1/HubFinance/compare/v0.0.3...v0.0.4
[0.0.3]: https://github.com/LuizFer1/HubFinance/compare/v0.0.2...v0.0.3
[0.0.2]: https://github.com/LuizFer1/HubFinance/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/LuizFer1/HubFinance/releases/tag/v0.0.1
