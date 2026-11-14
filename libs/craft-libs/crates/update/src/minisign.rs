//! Minisign signatures over release assets, keyed by the suite's single signing key.
//!
//! The release workflow attaches a `<asset>.minisig` sidecar to every release asset, signed
//! with the suite key whose public counterpart is embedded in [`SUITE_PUBKEYS`]. Checking is
//! advisory for now — the transitional policy logs and moves on, so a missing, stale or even
//! invalid signature never blocks an update. Flipping to a hard refusal is a one-line change
//! in [`check`], to be made once every live release of the suite's apps is signed.

use std::io::Read;
use std::path::Path;

use minisign_verify::{PublicKey, Signature};

use crate::http::Http;
use crate::release::{Asset, Release};

/// The suite's public minisign keys (base64), oldest first. A key rotation APPENDS the new key
/// here — never replaces: signatures made by retired keys must keep verifying, and
/// [`verify_with`] accepts a match against any entry.
pub(crate) const SUITE_PUBKEYS: &[&str] = &[
    // Suite key 0C5B214C53E837EA, generated 2026-11. Public by design; the private counterpart
    // lives only in the repos' MINISIGN_KEY secret and the offline backups.
    "RWTqN+hTTCFbDCGCA1cEJRLp2FSluxIUiJfqdoAYwkwNNVH5MTYH8qSN",
];

/// Verifies the file at `path` against `sig_text` using any of `pubkeys`.
///
/// `sig_text` is the two-pair minisign format (untrusted comment, base64 signature, trusted
/// comment, base64 global signature). Pre-hashed signatures verify in streaming fashion; the
/// legacy full-load path is the fallback.
pub(crate) fn verify_with(pubkeys: &[&str], path: &Path, sig_text: &str) -> Result<(), String> {
    let sig = Signature::decode(sig_text).map_err(|e| format!("malformed signature ({e})"))?;
    let mut failure = format!("no usable public key among {} entries", pubkeys.len());
    for key in pubkeys {
        let Ok(pk) = PublicKey::from_base64(key) else {
            failure = "an embedded public key is malformed".into();
            continue;
        };
        match verify_one(&pk, &sig, path) {
            Ok(()) => return Ok(()),
            Err(e) => failure = e,
        }
    }
    Err(failure)
}

fn verify_one(pk: &PublicKey, sig: &Signature, path: &Path) -> Result<(), String> {
    if let Ok(mut verifier) = pk.verify_stream(sig) {
        let mut file = std::fs::File::open(path).map_err(|e| format!("couldn't open the file ({e})"))?;
        let mut buf = [0u8; 64 * 1024];
        loop {
            let n = file.read(&mut buf).map_err(|e| format!("couldn't read the file ({e})"))?;
            if n == 0 {
                break;
            }
            verifier.update(&buf[..n]);
        }
        verifier.finalize().map_err(|e| format!("signature doesn't match ({e})"))
    } else {
        let data = std::fs::read(path).map_err(|e| format!("couldn't read the file ({e})"))?;
        pk.verify(&data, sig, false).map_err(|e| format!("signature doesn't match ({e})"))
    }
}

/// Advisory signature check for a downloaded asset (transitional policy: log, never block).
pub(crate) fn check(release: &Release, asset: &Asset, dest: &Path, http: &Http) {
    let Some(sig) = release.minisig_asset(&asset.name) else {
        log::warn!("update signature missing for {} (release {} predates suite signing)", asset.name, release.tag);
        return;
    };
    let text = match http.get_text(&sig.url) {
        Ok(Some(text)) => text,
        Ok(None) => {
            log::warn!("update signature for {} is listed but not downloadable", asset.name);
            return;
        }
        Err(e) => {
            log::warn!("couldn't fetch the signature for {} ({e})", asset.name);
            return;
        }
    };
    match verify_with(SUITE_PUBKEYS, dest, &text) {
        Ok(()) => log::info!("update signature verified for {}", asset.name),
        Err(e) => log::error!("update signature INVALID for {}: {} — proceeding only under the transitional policy", asset.name, e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Throwaway keypair (empty passphrase) committed with the fixtures; NOT a suite key.
    const FIXTURE_PUB: &str = "RWTLtmPo61hqXfOcI3S90J9kc0YY0S6JO+X/jkJiQ6mhS0bUjN4d8rnx";
    /// A second throwaway key, used to prove wrong-key rejection and the rotation model.
    const OTHER_PUB: &str = "RWSHbkBu1ve7lqm1WirM7uVg/TAhXil1FY+PLdFaiFHcJhZh/pgry8KG";

    const GOOD_SIG: &str = include_str!("../tests/fixtures/minisign/sample.txt.minisig");
    const WRONG_KEY_SIG: &str = include_str!("../tests/fixtures/minisign/sample-wrong.txt.minisig");

    fn fixture(name: &str) -> std::path::PathBuf {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/minisign").join(name)
    }

    #[test]
    fn verifies_a_prehashed_signature() {
        assert!(verify_with(&[FIXTURE_PUB], &fixture("sample.txt"), GOOD_SIG).is_ok());
    }

    #[test]
    fn rejects_a_tampered_file() {
        let tampered = std::env::temp_dir().join("craft-update-minisign-tampered");
        std::fs::write(&tampered, b"tampered contents\n").unwrap();
        assert!(verify_with(&[FIXTURE_PUB], &tampered, GOOD_SIG).is_err());
    }

    #[test]
    fn rejects_a_signature_from_another_key_but_rotations_accept_it() {
        assert!(verify_with(&[FIXTURE_PUB], &fixture("sample.txt"), WRONG_KEY_SIG).is_err());
        assert!(verify_with(&[FIXTURE_PUB, OTHER_PUB], &fixture("sample.txt"), WRONG_KEY_SIG).is_ok());
    }

    #[test]
    fn rejects_malformed_signature_text() {
        assert!(verify_with(&[FIXTURE_PUB], &fixture("sample.txt"), "not a signature").is_err());
    }

    #[test]
    fn suite_pubkeys_parse() {
        for key in SUITE_PUBKEYS {
            assert!(PublicKey::from_base64(key).is_ok(), "SUITE_PUBKEYS entry {key} must parse");
        }
    }
}
