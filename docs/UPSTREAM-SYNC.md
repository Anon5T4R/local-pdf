# Sincronização com o upstream

Porta/rebrand da suíte Local: o main é nosso, o upstream é fonte de mudanças candidatas e nada entra sem revisão. Regras gerais em `docs/planos/licoes-de-porta.md` (dev-notes).

## Base

- Upstream: `storytold/pdfcraft`, commit **64e0868b** (`feat(ui): menus from craft-menu, native on
  macOS (#4752)`, 2026-10-26). A árvore partiu desse commit no fork (`f8e52c5`, 26/out), sem
  histórico.
- Prova (conferida em 2026-11-21 no clone raso de `C:\Dev\storytold\pdfcraft`): carregamos a mudança
  de `64e0868b` — 8 dos 9 arquivos testáveis de `01b696df2..64e0868b` batem (ex.:
  `crates/ui-egui/tests/it/editing.rs`; o nono é `Cargo.toml`, que é nosso) — e **nenhum** dos 46
  arquivos testáveis dos 11 commits seguintes do `main` entrou. O primeiro filho, `d28d047a9` (split
  do engine), não está: `crates/engine/src/tests.rs` aqui ainda importa só `super::*`, sem
  `super::document::is_placeholder_title`/`super::edit::check_permission`.
- Os hashes citados no `ROADMAP.md` (`99941daa`, `33c14e07`, `a6bc63a`) vêm das entradas herdadas do
  upstream (rebases internos deles); o `merge-base` dos três é `33c14e07` (10/out) e não diz nada
  sobre a nossa base — a base é 16 dias depois, no `main` de 26/out.
- Fork: `Styx-Studios/Local-PDF`, versão própria na linha `0.14.x`.
- Nada é lido de `C:\Dev\storytold` em build, teste ou release; a pasta é só referência local para a
  skill `sync-upstream`.

## Vendorizado

Fork de upstream que este app vendoriza e cujo sync este documento cobre; os demais forks vendored
(`craft-libs`, `egui`, `winit`, `local-ai`) têm o rev no `AGENTS.md`.

| Crate | Upstream | Rev | O que entrou |
|-------|----------|-----|--------------|
| `libs/winprint.rs` | `storytold/winprint.rs` | `f084d35` (2026-11-21) | job de PDF por stream (`17f5f1e`, `b334ebb`, `03a4fb6`, `800a6f9`), página renderizada na resolução do job (`6f3012d`), fábrica Direct2D multi-thread (`f256775`), ticket XML (`2390a37`, `cbffb07`), `PrintTicket::from_dev_mode` e `PrinterDevice::is_xps_driver` sem uso por ora (`8ae097e`, `bc06a7c`) e o teardown das portas de arquivo dos testes (`f084d35`) |

Ficaram de fora: os commits de CI, StepSecurity e Harden-Runner do upstream.

`6758393` (build-dependencies do download do PDFium opcionais atrás da feature `pdfium`, com o código do
`build.rs` num módulo sob a mesma feature) entrou à parte do rev acima. O app usa o `winprint` com
`default-features = false`; `reqwest`, `sigstore-*` e `aws-lc-sys` deixaram de ser compilados.

## craft-update vendored: fork permanente da suíte (fora do sync)

`libs/craft-libs/crates/update` está **fora do escopo** deste sync com o upstream. É um crate nosso, hospedado no `craft-libs` da suíte Local, e um **fork permanente consciente** do `craft-update` upstream — unificado nos 10 apps em 2026-11-13, com um diff de milhares de linhas contra `storytold/craft-libs/crates/update` (MSI context/migrate, pins do launcher, `offers_update`, `desktop_register`, catálogo único de apps). Nenhum diff de sync encaixa nele.

Bugfix e feature vão **direto na fonte única** — repo `Styx-Studios/local-libs` (crate `crates/update`) — e daí são replicados para os apps pelo script de sincronização (`sync.ps1 -Mode push`, guarda de hash ×10; a cópia vendored aqui é espelho). Nunca edite a cópia local diretamente: o próximo sync sobrescreve. Contribuições ao upstream só via **Issue/PR manual**, em inglês, quando o problema também existir lá - mesma política do `UPSTREAM-CONTRIBUTIONS.md` do local-office. O restante do `craft-libs` segue o fluxo normal deste documento.
