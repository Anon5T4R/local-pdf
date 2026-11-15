# local-ai

BYOK AI compartilhado da suíte Local: as chaves do próprio usuário, um cliente
OpenAI-compatible para todos os providers (Gemini, Groq, OpenAI, OpenRouter,
gateways OpenCode). Nada conversa com servidor nenhum — não existe.

Conexão = preset de provider (base URL editável) + modelo buscado do `/models`
do próprio provider + segredo no keychain do SO (`keyring`, v1).

Cada app da suíte carrega a própria identidade via [`App`]: pasta de config
(`<data dir>/<id>/ai-connections.json`), serviço do keychain e URL do projeto
nos headers do OpenRouter. Extraído do `localpdf-ai` (LocalPDF) — apps da
suíte fazem um wrapper fino com a identidade deles e mantêm a API antiga.
