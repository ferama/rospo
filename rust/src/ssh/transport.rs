use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use internal_russh_forked_ssh_key::PublicKey;
use russh::client;
use russh::client::Handle;

use super::types::{ClientHandler, KeyGrabber};
use super::LOG;

pub async fn fetch_server_public_key(server: (&str, u16)) -> Result<PublicKey, String> {
    LOG.log(format_args!("grabbing server public key from {}:{}", server.0, server.1));
    let config = Arc::new(client::Config {
        inactivity_timeout: Some(Duration::from_secs(5)),
        ..Default::default()
    });
    let handler = KeyGrabber::default();
    let captured = handler.server_key.clone();
    let session = client::connect(config, server, handler)
        .await
        .map_err(|err| err.to_string())?;
    session
        .disconnect(russh::Disconnect::ByApplication, "", "English")
        .await
        .map_err(|err| err.to_string())?;

    captured
        .lock()
        .map_err(|_| "failed to acquire server key lock".to_string())?
        .clone()
        .ok_or_else(|| "server did not present a public key".to_string())
}

pub fn load_secret_key(path: &Path, password: Option<&str>) -> Result<Arc<russh::keys::PrivateKey>, String> {
    russh::keys::load_secret_key(path, password)
        .map(Arc::new)
        .map_err(|err| err.to_string())
}

pub(crate) fn build_client_config() -> Arc<client::Config> {
    Arc::new(client::Config {
        inactivity_timeout: None,
        keepalive_interval: Some(Duration::from_secs(5)),
        keepalive_max: 3,
        nodelay: true,
        ..Default::default()
    })
}

pub(crate) async fn authenticate_handle(
    handle: &mut Handle<ClientHandler>,
    username: &str,
    identity: &Path,
    password: Option<&str>,
) -> Result<(), String> {
    let mut authenticated = false;
    if let Ok(key) = load_secret_key(identity, None) {
        let auth = handle
            .authenticate_publickey(
                username.to_string(),
                russh::keys::PrivateKeyWithHashAlg::new(
                    key,
                    handle.best_supported_rsa_hash().await.map_err(|err| err.to_string())?.flatten(),
                ),
            )
            .await
            .map_err(|err| err.to_string())?;
        authenticated = auth.success();
    }

    if !authenticated && let Some(password) = password {
        let auth = handle
            .authenticate_password(username.to_string(), password.to_string())
            .await
            .map_err(|err| err.to_string())?;
        authenticated = auth.success();
    }

    if !authenticated {
        let auth = handle
            .authenticate_none(username.to_string())
            .await
            .map_err(|err| err.to_string())?;
        authenticated = auth.success();
    }

    if authenticated {
        Ok(())
    } else {
        Err("authentication failed".to_string())
    }
}
