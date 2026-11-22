# AGENTS.md — Local PDF

Fork independente do pdfcraft (ArtCraft), rebrandado para a suíte Local do João.
Rust puro + egui. Sem conta, sem telemetria, sem dependência de servidores de terceiros.

## Regras
- **Dependências:** toda dep que entra/sai/atualiza exige `Cargo.lock` commitado +
  linha no inventário do repo `Anon5T4R/local-deps` (regra completa em
  `local-deps/WORKFLOW.md`). PR sem isso não mergea.
- **Sem fontes git:** nenhuma dependência pode apontar para `git+` (o CI valida).
  Tudo vem do crates.io (pinado pelo lock) ou de `libs/`/`vendor/` (vendored).
- **Commits:** português, sem Co-Authored-By.
- **Release:** tag `vX.Y.Z` → o Actions builda Windows + Linux e publica. Mínimo local.
- **Disco:** no workspace do João vale a política de `target/` do dev-notes (`MEMORY.md` §4 — limpa o que
  está frio ou feito, mantém durante iteração ativa); fora dele, `cargo clean` ao terminar. Builds pesados são do CI.

## Estrutura
- `crates/` — engine e módulos (`localpdf-*`)
- `apps/localpdf` — app desktop; `apps/localpdf-cli` — CLI/MCP
- `libs/` — forks vendored: `craft-libs`, `egui`, `winit`, `winprint.rs`, `local-ai`
  (BYOK — chave de API/config, consumido pelo `crates/ai`; fonte em
  `C:\Dev\underworld\local-ai`, NÃO é o app LocalAI da suíte antiga)
- `vendor/` — patches de terceiros (hayro, lopdf, vello_common, provenance winit/egui-wgpu)
- `packaging/` — Windows (WiX/MSI), Linux (AppImage/deb/rpm/tar FHS com `install.sh`), Arch (PKGBUILD), macOS
- `scripts/check-mirror.sh` — confere as crates e os assets (`craft-fonts`) espelhados do `local-libs` contra a fonte (CI e `xtask ci`)
- `xtask/` — tarefas de build/CI (`cargo xtask ci`, `fmt`)

## Agent-native (MCP)
O app é dirigido por agente por dois caminhos: `localpdf-cli mcp` (headless, os ~141 tools de
`crates/automation`) e `localpdf-cli mcp --connect CONTROL_FILE` (live: o app aberto com
`localpdf --control FILE`, por TCP loopback com token no arquivo). O modo live é servido pela crate
compartilhada **`craft-mcp`** (`libs/craft-libs/crates/mcp`, fonte no repo `local-libs`, espelhada
por `sync.ps1 -Crate mcp`); o headless continua em `crates/automation`. `run --script` aceita JSON
array ou JSON Lines; `tools --search TEXT` filtra o catálogo. Docs: `docs/mcp.md`.

## Build
```
cargo build --locked          # debug
cargo test --locked           # testes (as fontes vendored entram sempre: .cargo/config.toml, igual ao CI)
cargo xtask ci                # o gate local: fmt, clippy, testes, camadas, assets,
                              # cargo-deny, espelhos do local-libs, parity
packaging/windows/package.ps1 # MSI + portable (Windows)
packaging/linux/package.sh    # AppImage/deb/rpm/tar (Linux)
```


## Upstream e a pasta `C:\Dev\storytold`

Menções a `storytold/*` em código, comentários, docs e atribuições (NOTICE/ATTRIBUTION) são referência histórica e obrigação de licença. **Nada** é lido de `C:\Dev\storytold` em build, teste ou release: tudo vem do próprio repo (crates.io pinado pelo lock + `libs/`/`vendor/` vendored + CI que busca craft-fonts/craft-i18n do GitHub). Essa pasta existe somente como referência local para a skill `sync-upstream`.
