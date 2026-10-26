# AGENTS.md — LocalPDF

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
- **Disco local:** nunca deixe `target/` para trás — `cargo clean` após uso local;
  builds pesados são do CI.

## Estrutura
- `crates/` — engine e módulos (`localpdf-*`)
- `apps/localpdf` — app desktop; `apps/localpdf-cli` — CLI/MCP; `apps/localpdf-web` — wasm
- `libs/` — forks vendored: `craft-libs`, `egui`, `winit`, `winprint.rs`
- `vendor/` — patches de terceiros (hayro, lopdf, vello_common, provenance winit/egui-wgpu)
- `packaging/` — Windows (WiX/MSI), Linux (AppImage/deb/rpm/tar), Arch (PKGBUILD), macOS, web
- `xtask/` — tarefas de build/CI (`cargo xtask ci`)

## Build
```
cargo build --locked          # debug
cargo test --locked           # testes
packaging/windows/package.ps1 # MSI + portable (Windows)
packaging/linux/package.sh    # AppImage/deb/rpm/tar (Linux)
```
