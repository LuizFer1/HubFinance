# CLAUDE.md — HubFinance

Hub de sync do HomeFinance: programa desktop em Rust (iced 0.14) que roda no computador do
usuário, guarda a versão mais recente de cada linha que os celulares enviam e devolve a cada um
o que ele ainda não viu. Chame de **hub**, nunca de servidor.

A documentação (arquitetura, specs, planos) fica em `D:\Projetos Pessoais\HomeFinance\docs\desktop\`,
fora deste repositório. Leia `ARCHITECTURE.md` antes de tocar em `src/`.

## Comandos

`cargo run` · `cargo test` · `cargo clippy --all-targets -- -D warnings` · `cargo fmt --check`.
`RUST_LOG=debug cargo run` para log detalhado; o log vai para o console **e** para
`<data_dir>/logs/hub.AAAA-MM-DD.log` (7 arquivos, rotação diária). Nunca logar chave, hash,
token de pareamento nem linha inteira — a atividade da janela é espelhada no arquivo.
`HUBFINANCE_DATA_DIR=<dir>` muda o diretório de dados (padrão: `%LOCALAPPDATA%\HubFinance\data`).

`scripts/freeze-inter.sh` e `scripts/oklch.py` são geração offline (fontes com `tnum` congelado
e tabela de cores); o resultado é commitado e `assets/fonts` é vendorizado com as licenças. O
binário nunca baixa nada. `scripts/e2e.sh` e `scripts/seed-dashboard.sh <ip> <token>` falam
com um hub de verdade por curl.

## Restrições de produto (inegociáveis)

- O hub **nunca edita dado**: toda mutação nasce num celular; o hub só aplica LWW por linha.
- **Nada fora da LAN.** Nenhuma dependência que telefone para casa, nenhum relay, nenhuma
  checagem de versão **pelo processo do hub**: abrir o navegador numa URL fixa é ação do
  usuário executada pelo sistema.
- **Opcional.** App com sync desligado é completo; hub desligado não degrada nenhum celular.
- **Uma regra para todas as tabelas.** Linhas são JSON opaco; tabela ou campo novo no celular
  sincroniza sem código novo aqui.

## Invariantes

- HLC tem largura fixa (`millis(13)-counter(4 hex)-deviceId(26)`); comparação de string é a
  comparação semântica. Nunca parseie para comparar.
- `id` são 26 caracteres Crockford — **não** ULID estrito: ids determinísticos do app usam o
  alfabeto inteiro na primeira posição.
- `seq` só é atribuído a linha aceita, dentro da transação do lote, e só cresce.
- `dirty` é removido ao armazenar e nunca devolvido.
- `epoch` nasce com o banco e só muda se o banco for apagado.
- A chave do aparelho nunca é gravada: só `sha256` hex.
- Linha fora do contrato de campos é ignorada e contada no dashboard; nunca derruba sync nem
  janela.
- A versão é uma só: `version` do `Cargo.toml` (`config::VERSION`); a tag é `v<versão>` e
  `release.yml` falha se diferirem. Toda release tem seção no `CHANGELOG.md`.

## Layering

`protocol/` é puro (sem rusqlite, axum, tokio, iced). `store/` é o único módulo com rusqlite;
`server/` o único com axum; `ui/` o único com iced. `dashboard/` é puro (sem rusqlite, axum,
tokio, iced): contrato de campos, dataset e agregações; a UI deriva as telas dele. `hub/` liga
tudo numa thread com runtime tokio e publica `Snapshot` por `watch`; a UI só renderiza o
snapshot e envia `Command`. `logging.rs` e `main.rs` são os únicos com `tracing-subscriber`/
`tracing-appender`; `ui/` abre o navegador só por `open::that_detached` com URL constante de
`config.rs`.

## Convenções

- Comentários, commits e textos de UI em português; identificadores em inglês. Comentário
  explica *por que* a regra existe e o que quebra sem ela.
- Commits pequenos e convencionais (`feat(store):`, `fix(server):`, `test(protocol):`), assunto
  sem acento. **Sem co-autor** (`Co-Authored-By`) e sem "Generated with Claude Code". Autor é
  sempre `Luiz Fernando <luisferndantas@outlook.com>` (config local do repo); nunca `--author`
  nem `-c user.email`.
- `git add` por caminho explícito; nunca `-A` nem `.`. `Cargo.lock` é commitado.
- Erros com `thiserror`; sem `unwrap`/`expect` fora de testes e de `main`.
- Testes colocados em `#[cfg(test)] mod tests` no próprio arquivo; sem rede, sem janela, sem o
  diretório do usuário (use `tempfile`).
