# Local PDF — estado e fila

Fork independente do pdfcraft (ArtCraft), rebrandado para a suíte Local. Versão e release notes:
`cargo xtask version`, tags e GitHub Releases. Onde cada coisa mora: `AGENTS.md`.

## O que está pronto

- **Ler e navegar**: abrir PDF (inclusive protegido por senha), busca, painéis (comentários,
  marcadores, páginas, formulários, camadas, anexos, assinaturas, acessibilidade, tags), modos de
  exibição (uma página, contínuo, duas páginas) e impressão direta no desktop (`craft-print`).
- **Editar**: texto existente e figuras na página, mover/duplicar vários objetos, redigir, marcar
  com carimbo, marca d'água, cabeçalho/rodapé e fundo; organizar páginas (inserir, extrair,
  dividir, combinar, girar).
- **Formulários**: AcroForm completo (preencher, achatar, exportar/importar XFDF) e XFA 3.3
  estático e dinâmico (`crates/xfa`); JavaScript de formulário por um subconjunto próprio
  (`crates/js`).
- **Documento**: assinatura digital, comparação entre arquivos, otimização/compactação, OCR,
  exportar imagens/texto, PDF/A, auditoria de acessibilidade e de pré-impressão.
- **Agente nativo**: `localpdf-cli mcp` (headless, `crates/automation`) e `mcp --connect` no app
  aberto, servido pelo `craft-mcp` (`docs/mcp.md`, `docs/cli-batch.md`).
- **Casca no padrão da suíte**: barra de modo com abas (All tools, Edit, Convert, E-Sign), painel de
  IA, paleta, temas Automático/Claro/Escuro, cartão de atualização do `craft-update`.

## O que falta

Na ordem em que dói no uso diário (levantado em 2026-10, e desde então corrigido por partes):

1. **Fidelidade em PDFs difíceis**: quem lê a especificação toda é `crates/cos`/`crates/render`;
   páginas com construções raras (fontes Type 3 malformadas, cores não suportadas, estruturas
   danificadas) ainda divergem do Acrobat e não há comparação página a página automatizada.
2. **XFA**: cobertura dos formulários reais ainda incompleta (`crates/xfa`).
3. **JavaScript de documento**: o subconjunto de `crates/js` não cobre todas as APIs do Acrobat.
4. **Interface espelhada (RTL)** e os catálogos de tradução ainda incompletos em alguns idiomas.
5. **Plataformas**: este fork publica Windows e Linux; macOS, BSD e Flatpak não são publicados.

Itens de release, CI e gate local: `AGENTS.md` (§ Regras, § Build) e `docs/git-protocol.md` do
dev-notes. Decisões do sync com o upstream: `docs/UPSTREAM-SYNC.md`.
