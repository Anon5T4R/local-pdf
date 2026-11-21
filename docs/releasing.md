# Releasing Local PDF

O fluxo da suíte (gate local, tag, CI) está em `docs/git-protocol.md` do dev-notes; aqui ficam só os
detalhes deste app.

1. Bump: `cargo xtask version set X.Y.Z` (a fonte única é `[workspace.package] version`).
2. Commit, tag `vX.Y.Z`, push. `.github/workflows/release.yml` builda e publica:
   Windows (`.msi` por máquina e por usuário, `.zip` portátil) e Linux (`.AppImage`, `.deb`, `.rpm`,
   `.tar.gz` FHS com `install.sh`), com `SHA256SUMS.txt`.
3. Assinatura: os segredos vivem no environment `release` do repositório; sem eles o build sai sem
   assinatura, com aviso (é o que permite um fork buildar).

Empacotamento local: `packaging/windows/package.ps1` e `packaging/linux/package.sh`. Nenhuma outra
plataforma é publicada por este fork.
