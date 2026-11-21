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
- Fork: `Anon5T4R/local-pdf`, versão própria na linha `0.14.x`.
- Nada é lido de `C:\Dev\storytold` em build, teste ou release; a pasta é só referência local para a
  skill `sync-upstream`.

## craft-update vendored: fork permanente da suíte (fora do sync)

`libs/craft-libs/crates/update` está **fora do escopo** deste sync com o upstream. É um crate nosso, hospedado no `craft-libs` da suíte Local, e um **fork permanente consciente** do `craft-update` upstream — unificado nos 10 apps em 2026-11-13, com um diff de milhares de linhas contra `storytold/craft-libs/crates/update` (MSI context/migrate, pins do launcher, `offers_update`, `desktop_register`, catálogo único de apps). Nenhum diff de sync encaixa nele.

Bugfix e feature vão **direto na fonte única** — repo `Anon5T4R/local-libs` (crate `crates/update`) — e daí são replicados para os apps pelo script de sincronização (`sync.ps1 -Mode push`, guarda de hash ×10; a cópia vendored aqui é espelho). Nunca edite a cópia local diretamente: o próximo sync sobrescreve. Contribuições ao upstream só via **Issue/PR manual**, em inglês, quando o problema também existir lá - mesma política do `UPSTREAM-CONTRIBUTIONS.md` do local-office. O restante do `craft-libs` segue o fluxo normal deste documento.
