//! // craft-update: provider cripto do rustls no caminho do craft-update.
//!
//! As requisições de update (check de
//! release e download do instalador) usam o provider **100% Rust** (`rustls-rustcrypto`, fork
//! vendored em `libs/rustls-rustcrypto`) como caminho **ÚNICO**. Um handshake que falhe vira erro
//! normal - visível na linha de status do app (botão "Verificar atualizações") e no log de sessão -
//! em vez de cair calado para outro provider.
//!
//! Não existe válvula de fallback: nenhum provider C (`ring`/`aws-lc-rs`) é compilado no caminho do
//! update. Se um servidor do release parar de negociar com este provider, o erro aparece no app (e
//! no log) em vez de ser mascarado por um retry silencioso em outro backend.

use std::sync::Arc;

use rustls::crypto::CryptoProvider;

/// O provider puro que o ureq recebe em cada agente. Uma instância por conjunto de agentes: o
/// `CryptoProvider` carrega a lista de suites/algoritmos, não guarda estado de sessão.
pub fn pure_provider() -> Arc<CryptoProvider> {
    Arc::new(rustls_rustcrypto::provider())
}

/// Linha para o log de sessão do app: sempre `tls_provider=pure` (não há troca de provider).
pub fn status_line() -> String {
    "tls_provider=pure".into()
}

/// Diagnóstico para um teste de app (`tests/tls_update_check.rs`, opcional): um handshake real com
/// `host:443` usando o provider puro, devolvendo a versão de protocolo e a suite que o servidor
/// negociou. Não é chamado em produção.
pub fn probe_pure(host: &str) -> Result<String, String> {
    let provider = pure_provider();
    let verifier = rustls_platform_verifier::Verifier::new(provider.clone()).map_err(|e| format!("verifier: {e}"))?;
    let config = rustls::ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(rustls::ALL_VERSIONS)
        .map_err(|e| format!("builder: {e}"))?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(verifier))
        .with_no_client_auth();
    let name = rustls::pki_types::ServerName::try_from(host.to_string()).map_err(|e| format!("host inválido: {e}"))?;
    let mut conn = rustls::ClientConnection::new(Arc::new(config), name).map_err(|e| format!("client: {e}"))?;
    let mut sock = std::net::TcpStream::connect((host, 443u16)).map_err(|e| format!("connect: {e}"))?;
    let _ = sock.set_read_timeout(Some(std::time::Duration::from_secs(15)));
    while conn.is_handshaking() {
        conn.complete_io(&mut sock).map_err(|e| format!("handshake: {e}"))?;
    }
    let version = conn.protocol_version().map_or_else(|| "-".to_string(), |v| format!("{v:?}"));
    let suite = conn.negotiated_cipher_suite().map_or_else(|| "-".to_string(), |s| format!("{:?}", s.suite()));
    Ok(format!("{version} {suite}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// O log de sessão identifica o caminho único.
    #[test]
    fn the_session_line_names_the_single_provider() {
        assert_eq!(status_line(), "tls_provider=pure");
    }
}
