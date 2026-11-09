# rustls-rustcrypto (fork vendored do Local PDF)

Provider criptográfico 100% Rust para o `rustls`, baseado nos crates da
RustCrypto. Este é um **fork vendored** usado só pelo app Local PDF (o
experimento TLS puro do updater) — não é (ainda) o padrão da suíte craft. A
cópia fica em `libs/rustls-rustcrypto` do próprio app, nunca dentro do
`libs/craft-libs`, e é sincronizada à mão entre os apps que adotam o experimento.

- Fonte: `rustls-rustcrypto 0.0.2-alpha` do crates.io
  (<https://github.com/RustCrypto/rustls-rustcrypto>), licença MIT OR Apache-2.0
  (os LICENSE-* estão aqui e o README original ficou em `UPSTREAM-README.md`).
- Por que existe: o upstream 0.0.2-alpha depende de `rustls-webpki 0.102` só
  para as constantes `alg_id` (identificador DER do algoritmo de assinatura).
  Isso arrastava 5 advisories de webpki 0.102 para o `cargo deny` do app, sem
  como corrigir pela registry. Este fork resolve o problema localmente.

## Divergências (todas marcadas com `// local-pdf:` no código)

| Arquivo | Mudança |
| --- | --- |
| `Cargo.toml` | sai a dependência `rustls-webpki 0.102` (e os features `webpki/*`); `rustls-pki-types` sobe para `1.14` (onde moram as constantes `alg_id`); `[workspace]` para o crate ser workspace próprio fora do `libs/` do app |
| `src/verify/{rsa,ecdsa,eddsa}.rs` | `use webpki::alg_id;` → `use pki_types::alg_id;` |

As constantes usadas (`RSA_ENCRYPTION`, `RSA_PKCS1_SHA*`, `RSA_PSS_SHA*`,
`ECDSA_P256`, `ECDSA_P384`, `ECDSA_SHA*`, `ED25519`) têm exatamente os mesmos
bytes DER nos dois crates (os `data/alg-*.der` são idênticos), então a troca não
muda nenhuma verificação — só remove o webpki 0.102 do grafo.

Nada mais do upstream foi tocado. Ao sincronizar com uma release nova do
`rustls-rustcrypto`, refaça só esses dois pontos.
