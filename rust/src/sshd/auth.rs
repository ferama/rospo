use std::collections::HashSet;
use std::path::Path;

use internal_russh_forked_ssh_key::PublicKey as ParsedPublicKey;
use internal_russh_forked_ssh_key::{Algorithm, EcdsaCurve, LineEnding};
use p521::elliptic_curve::rand_core::OsRng;
use tokio::fs;

use crate::logging::{Logger, BLUE};
use crate::utils::{expand_user_home, write_file_0600};

const LOG: Logger = Logger::new("[SSHD] ", BLUE);

pub(super) async fn ensure_server_key(path: &Path) -> Result<(), String> {
    if path.exists() {
        return Ok(());
    }
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).await.map_err(|err| err.to_string())?;

    let secret = russh::keys::PrivateKey::random(
        &mut OsRng,
        Algorithm::Ecdsa {
            curve: EcdsaCurve::NistP521,
        },
    )
    .map_err(|err| err.to_string())?;
    let private_pem = secret
        .to_openssh(LineEnding::LF)
        .map_err(|err| err.to_string())?
        .to_string();
    let public = secret
        .public_key()
        .to_openssh()
        .map_err(|err| err.to_string())?;

    write_file_0600(path, private_pem.as_bytes())?;
    write_file_0600(&path.with_extension("pub"), format!("{public}\n").as_bytes())?;
    Ok(())
}

pub(super) async fn is_authorized_key(
    sources: &[String],
    public_key: &russh::keys::ssh_key::PublicKey,
) -> bool {
    match load_authorized_keys(sources).await {
        Ok(keys) => keys.contains(public_key),
        Err(_) => false,
    }
}

pub(super) async fn load_authorized_keys(
    sources: &[String],
) -> Result<HashSet<russh::keys::ssh_key::PublicKey>, String> {
    let mut keys = HashSet::new();
    for source in sources {
        // Missing or temporarily unreachable sources are skipped so one bad URL or path does not
        // prevent the rest of the configured authorized_keys inputs from loading.
        let content = match load_authorized_keys_source(source).await {
            Ok(content) => content,
            Err(_) => continue,
        };
        merge_authorized_keys(&mut keys, &content)?;
    }
    Ok(keys)
}

async fn load_authorized_keys_source(source: &str) -> Result<String, String> {
    if is_http_source(source) {
        LOG.log(format_args!("loading keys from http {}", source));
        let response = reqwest::get(source).await.map_err(|err| err.to_string())?;
        let response = response.error_for_status().map_err(|err| err.to_string())?;
        response.text().await.map_err(|err| err.to_string())
    } else {
        LOG.log(format_args!("loading keys from file {}", source));
        fs::read_to_string(expand_user_home(source))
            .await
            .map_err(|err| err.to_string())
    }
}

fn merge_authorized_keys(
    keys: &mut HashSet<russh::keys::ssh_key::PublicKey>,
    content: &str,
) -> Result<(), String> {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        // Ignore options/comments syntax we do not currently model and only keep lines that parse
        // as standalone OpenSSH public keys, which matches the accepted sources in the Go code.
        if let Ok(key) = ParsedPublicKey::from_openssh(trimmed) {
            let openssh = key.to_openssh().map_err(|err| err.to_string())?;
            let parsed =
                russh::keys::ssh_key::PublicKey::from_openssh(&openssh).map_err(|err| err.to_string())?;
            keys.insert(parsed);
        }
    }
    Ok(())
}

fn is_http_source(source: &str) -> bool {
    match reqwest::Url::parse(source) {
        Ok(url) => matches!(url.scheme(), "http" | "https"),
        Err(_) => false,
    }
}

pub(super) fn normalize_bind_address(address: &str, port: u16) -> String {
    if address.is_empty() {
        return format!("0.0.0.0:{port}");
    }
    if address.contains(':') && !address.starts_with('[') {
        return format!("[{address}]:{port}");
    }
    format!("{address}:{port}")
}

pub(super) fn normalize_server_listen_address(address: &str) -> String {
    if let Some(port) = address.strip_prefix(':') {
        return format!("0.0.0.0:{port}");
    }
    address.to_string()
}

pub(super) fn forward_key(address: &str, port: u32) -> String {
    format!("{address}:{port}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn is_http_source_detects_supported_schemes() {
        assert!(is_http_source("http://127.0.0.1:8080/authorized_keys"));
        assert!(is_http_source("https://example.com/user.keys"));
        assert!(!is_http_source("/tmp/authorized_keys"));
        assert!(!is_http_source("ftp://example.com/user.keys"));
    }

    #[test]
    fn merge_authorized_keys_skips_comments_and_blank_lines() {
        let content = format!(
            "# comment\n\n{}\n",
            include_str!("../../../testdata/client.pub").trim()
        );
        let mut keys = HashSet::new();

        merge_authorized_keys(&mut keys, &content).expect("merge should parse valid authorized_keys");

        assert_eq!(keys.len(), 1);
    }

    #[test]
    fn normalize_server_listen_address_supports_go_style_port_binding() {
        assert_eq!(normalize_server_listen_address(":2222"), "0.0.0.0:2222");
        assert_eq!(
            normalize_server_listen_address("127.0.0.1:2222"),
            "127.0.0.1:2222"
        );
    }

    #[tokio::test]
    async fn load_authorized_keys_reads_local_file_sources() {
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../testdata/authorized_keys")
            .display()
            .to_string();

        let keys = load_authorized_keys(&[source]).await.expect("authorized_keys should load");

        assert!(!keys.is_empty());
    }
}
