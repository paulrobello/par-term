//! Detached signature verification for downloaded release artifacts (SEC-005).
//!
//! # Why a signature and not just the SHA256
//!
//! The release binary and its `.sha256` file are two assets in the same GitHub
//! release, fetched from the same `assets` array through the same download path.
//! Anyone able to replace the binary can replace the checksum alongside it, so
//! the checksum defends against *corruption in transit*, not against a
//! *compromised release*. A detached signature made with a key that never exists
//! on the release runner is the control that actually distinguishes the two.
//!
//! Verification here is **fail-closed** in every direction: an unconfigured key,
//! a malformed key, a missing signature asset, a malformed signature, a
//! signature made by a different key, and a signature that simply does not match
//! all produce an error. There is no path through this module that returns
//! `Ok(())` for unverified bytes.
//!
//! # MAINTAINER: this build has no signing key yet
//!
//! [`UPDATE_SIGNING_PUBLIC_KEY`] is an intentionally empty placeholder, so
//! self-update currently refuses to install anything. Nothing in par-term
//! generates key material — the steps below are yours to run, once, on a machine
//! that is not the CI runner.
//!
//! 1. **Generate the keypair locally** (needs the `minisign` CLI, `brew install
//!    minisign`, or the Rust `rsign2` crate):
//!
//!    ```text
//!    minisign -G -p par-term-release.pub -s par-term-release.key
//!    ```
//!
//!    Keep `par-term-release.key` and its password offline. It must never be
//!    committed, and it must never be added to GitHub Actions secrets if the
//!    goal is to defend against a compromised release pipeline — a key the
//!    release runner can use is a key an attacker who owns the runner can use.
//!
//! 2. **Paste the public key** into [`UPDATE_SIGNING_PUBLIC_KEY`] below. Use only
//!    the base64 line of `par-term-release.pub` — the second line, not the
//!    `untrusted comment:` line above it.
//!
//! 3. **Sign each release asset** and publish the `.minisig` next to it:
//!
//!    ```text
//!    minisign -S -s par-term-release.key -m par-term-macos-aarch64.zip
//!    # produces par-term-macos-aarch64.zip.minisig
//!    ```
//!
//! 4. **Publish the assets.** As of v0.37.1 the release contains *neither* the
//!    per-binary `.sha256` files nor the `.minisig` files — only `shaders.zip`
//!    has a checksum. Both gates therefore reject every current release. The
//!    upload step that needs to carry the new files is the `gh release upload`
//!    in `.github/workflows/release.yml`; the per-binary `shasum -a 256` step
//!    that produces them does not exist yet either.
//!
//! Until steps 1–4 are done, self-update fails with an explanatory error and
//! users update by downloading a release manually. That is the intended
//! behaviour, not a regression: the checksum gate already refuses every current
//! release for the same reason.

use minisign_verify::{PublicKey, Signature};

/// Base64 minisign public key that release artifacts must verify against.
///
/// **PLACEHOLDER — the maintainer must fill this in.** See the module docs for
/// how to generate the keypair and what to paste here. An empty value means
/// "no key configured", which makes [`verify_detached`] refuse every update
/// rather than fall back to checksum-only verification.
///
/// The value is the single base64 line from `minisign -G`'s `.pub` file, e.g.
/// `RWQf6LRCGA9i53mlYecO4IzT51TGPpvWucNSCh1CBM0QTaLn73Y7GFO3` (that example is
/// the upstream minisign documentation's key, not par-term's — do not use it).
pub const UPDATE_SIGNING_PUBLIC_KEY: &str = "";

/// Whether this build has a release-signing public key compiled in.
///
/// Callers use this only to produce a clearer error message; verification
/// itself refuses an unconfigured key regardless.
pub fn signing_key_configured() -> bool {
    !UPDATE_SIGNING_PUBLIC_KEY.trim().is_empty()
}

/// The shared explanation for a build with no signing key.
fn not_configured_error() -> String {
    "This build of par-term has no release-signing public key compiled in, so a \
     downloaded update cannot be proven to come from the par-term maintainer.\n\
     Self-update is disabled rather than falling back to checksum-only \
     verification: the binary and its .sha256 come from the same release, so the \
     checksum cannot detect a compromised release.\n\
     Maintainer: set UPDATE_SIGNING_PUBLIC_KEY in par-term-update/src/signature.rs \
     and publish both the per-binary .sha256 and the .minisig assets from \
     .github/workflows/release.yml.\n\
     Please download the release manually from:\n\
     https://github.com/paulrobello/par-term/releases"
        .to_string()
}

/// Parse the compile-time public key.
///
/// Fails when the key is absent (the shipped placeholder) or malformed. A
/// malformed key is treated exactly like a missing one: it can never verify
/// anything, so accepting the update would mean skipping the gate.
fn signing_key() -> Result<PublicKey, String> {
    if !signing_key_configured() {
        return Err(not_configured_error());
    }

    PublicKey::from_base64(UPDATE_SIGNING_PUBLIC_KEY.trim()).map_err(|e| {
        format!(
            "The release-signing public key compiled into this build is not a \
             valid minisign public key: {}.\n\
             Update aborted — a key that cannot be parsed cannot verify anything.\n\
             Maintainer: UPDATE_SIGNING_PUBLIC_KEY must be the base64 line of a \
             `minisign -G` public key file, with no surrounding comment line.",
            e
        )
    })
}

/// Verify a detached minisign signature over `data`.
///
/// `signature_text` is the full text of the `.minisig` file, including its
/// `untrusted comment:` and `trusted comment:` lines — minisign authenticates
/// the trusted comment, so the file must be passed through intact.
///
/// # Errors
///
/// Returns an error when the compiled-in key is missing or malformed, when the
/// signature file cannot be parsed, when it was made by a different key, or
/// when it does not match `data`. Every one of those aborts the update.
pub fn verify_detached(data: &[u8], signature_text: &str) -> Result<(), String> {
    let public_key = signing_key()?;

    let signature = Signature::decode(signature_text).map_err(|e| {
        format!(
            "Could not parse the .minisig signature accompanying this release: {}.\n\
             Update aborted — the signature file may be truncated, corrupted, or \
             not a minisign signature at all.",
            e
        )
    })?;

    // `allow_legacy: false` requires the modern prehashed (`ED`) format. Legacy
    // signatures stream the whole file through Ed25519 directly; refusing them
    // keeps the accepted format to exactly what current minisign produces.
    public_key.verify(data, &signature, false).map_err(|e| {
        format!(
            "Signature verification FAILED for the downloaded update: {}.\n\
             The download does not carry a valid signature from the par-term \
             release key. It may have been tampered with, or the release may have \
             been signed with a different key than this build trusts.\n\
             Update aborted — nothing was installed.",
            e
        )
    })?;

    log::info!("Release signature verified against the pinned minisign public key");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A syntactically plausible but meaningless `.minisig` body.
    const GARBAGE_SIGNATURE: &str = "untrusted comment: signature from minisign secret key\n\
         RUQf6LRCGA9i57777777777777777777777777777777777777777777777777777777777777777\n\
         trusted comment: timestamp:1700000000\tfile:par-term\n\
         77777777777777777777777777777777777777777777777777777777777777777777777777==\n";

    #[test]
    fn placeholder_key_is_not_configured() {
        // The shipped default must stay empty. A committed key would mean key
        // material was generated by tooling rather than by the maintainer.
        assert!(
            !signing_key_configured(),
            "UPDATE_SIGNING_PUBLIC_KEY should ship empty; if you filled it in, \
             update this test to assert the real key parses instead"
        );
    }

    #[test]
    fn unconfigured_key_refuses_any_signature() {
        let err = verify_detached(b"payload", GARBAGE_SIGNATURE)
            .expect_err("an unconfigured key must refuse to verify");
        assert!(
            err.contains("no release-signing public key") || err.contains("Signature verification"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn unconfigured_key_error_names_what_the_maintainer_must_do() {
        let err = not_configured_error();
        assert!(
            err.contains("UPDATE_SIGNING_PUBLIC_KEY"),
            "error should name the constant to fill in: {err}"
        );
        assert!(
            err.contains(".sha256") && err.contains(".minisig"),
            "error should name both missing release assets: {err}"
        );
        assert!(
            err.contains("release.yml"),
            "error should name the workflow that publishes them: {err}"
        );
    }

    #[test]
    fn garbage_signature_is_rejected() {
        // Fails at the key gate today and at the signature gate once a real key
        // is configured — either way the update must not proceed.
        assert!(verify_detached(b"payload", GARBAGE_SIGNATURE).is_err());
    }

    #[test]
    fn empty_signature_is_rejected() {
        assert!(verify_detached(b"payload", "").is_err());
    }
}
