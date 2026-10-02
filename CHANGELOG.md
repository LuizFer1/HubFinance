# Changelog

Todas as mudanças relevantes do HubFinance ficam aqui. O formato segue o
[Keep a Changelog](https://keepachangelog.com/pt-BR/1.1.0/) e a numeração é
[semver](https://semver.org/lang/pt-BR/): a versão é a do `Cargo.toml`, e a tag é `v<versão>`.

## [Não lançado]

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

[Não lançado]: https://github.com/LuizFer1/HubFinance/compare/v0.0.1...HEAD
[0.0.1]: https://github.com/LuizFer1/HubFinance/releases/tag/v0.0.1
