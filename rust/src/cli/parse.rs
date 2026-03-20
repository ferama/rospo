use std::path::PathBuf;

use crate::config::{JumpHostConf, SshClientConf};
use crate::ssh::{ClientOptions, JumpHostOptions};
use crate::sshd::ServerOptions;
use crate::utils::{current_home_dir, expand_user_home, get_default_ssh_config_host, parse_ssh_url};

use super::app::{SshClientArgs, SshdSharedArgs};

pub(crate) fn client_options_from_cli(server: &str, ssh: &SshClientArgs) -> Result<ClientOptions, String> {
    let default_identity = format!("{}/.ssh/id_rsa", current_home_dir());
    let default_known_hosts = format!("{}/.ssh/known_hosts", current_home_dir());
    let user_identity = ssh
        .user_identity
        .as_deref()
        .map(expand_user_home)
        .unwrap_or(default_identity);
    let known_hosts = ssh
        .known_hosts
        .as_deref()
        .map(expand_user_home)
        .unwrap_or(default_known_hosts);

    build_client_options_from_server(
        server,
        user_identity,
        known_hosts,
        ssh.password.clone(),
        ssh.insecure,
        ssh.disable_banner,
        ssh.jump_host.clone(),
        ssh.user_identity.is_some(),
        ssh.known_hosts.is_some(),
        ssh.insecure,
        ssh.jump_host.is_some(),
    )
}

pub(crate) fn client_options_from_conf(conf: &SshClientConf) -> Result<ClientOptions, String> {
    let parsed = parse_ssh_url(&conf.server)?;
    let identity = if conf.identity.is_empty() {
        format!("{}/.ssh/id_rsa", current_home_dir())
    } else {
        expand_user_home(&conf.identity)
    };
    let known_hosts = if conf.known_hosts.is_empty() {
        format!("{}/.ssh/known_hosts", current_home_dir())
    } else {
        expand_user_home(&conf.known_hosts)
    };
    Ok(ClientOptions {
        username: parsed.username,
        host: parsed.host.trim_matches(&['[', ']'][..]).to_string(),
        port: parsed.port,
        identity: PathBuf::from(identity),
        known_hosts: PathBuf::from(known_hosts),
        password: if conf.password.is_empty() {
            None
        } else {
            Some(conf.password.clone())
        },
        insecure: conf.insecure,
        quiet: conf.quiet,
        jump_hosts: jump_host_options_from_conf(&conf.jump_hosts)?,
    })
}

pub(crate) fn server_options_from_cli(args: &SshdSharedArgs) -> ServerOptions {
    ServerOptions {
        server_key: args.sshd_key.clone(),
        authorized_keys: vec![args.sshd_authorized_keys.clone()],
        authorized_password: args.sshd_authorized_password.clone(),
        listen_address: args.sshd_listen_address.clone(),
        disable_shell: args.disable_shell,
        disable_banner: false,
        disable_auth: args.disable_auth,
        disable_sftp_subsystem: false,
        disable_tunnelling: false,
        shell_executable: String::new(),
    }
}

fn build_client_options_from_server(
    server: &str,
    mut user_identity: String,
    known_hosts: String,
    password: Option<String>,
    mut insecure: bool,
    disable_banner: bool,
    mut jump_host: Option<String>,
    user_identity_changed: bool,
    known_hosts_changed: bool,
    insecure_changed: bool,
    jump_host_changed: bool,
) -> Result<ClientOptions, String> {
    let parsed = if let Some(host_conf) = get_default_ssh_config_host(server) {
        if !user_identity_changed {
            user_identity = expand_user_home(&host_conf.identity_file);
        }
        let known_hosts = if !known_hosts_changed {
            expand_user_home(&host_conf.user_known_hosts_file)
        } else {
            known_hosts
        };
        if !jump_host_changed && !host_conf.proxy_jump.is_empty() {
            jump_host = Some(host_conf.proxy_jump.clone());
        }
        if !insecure_changed {
            insecure = !host_conf.strict_host_key_checking;
        }
        let parsed = parse_ssh_url(&format!("{}@{}:{}", host_conf.user, host_conf.host_name, host_conf.port))?;
        let jump_hosts = build_jump_hosts(jump_host, user_identity.clone())?;
        return Ok(ClientOptions {
            username: parsed.username,
            host: parsed.host.trim_matches(&['[', ']'][..]).to_string(),
            port: parsed.port,
            identity: PathBuf::from(user_identity),
            known_hosts: PathBuf::from(known_hosts),
            password,
            insecure,
            quiet: disable_banner,
            jump_hosts,
        });
    } else {
        parse_ssh_url(server)?
    };

    Ok(ClientOptions {
        username: parsed.username,
        host: parsed.host.trim_matches(&['[', ']'][..]).to_string(),
        port: parsed.port,
        identity: PathBuf::from(user_identity.clone()),
        known_hosts: PathBuf::from(known_hosts),
        password,
        insecure,
        quiet: disable_banner,
        jump_hosts: build_jump_hosts(jump_host, user_identity)?,
    })
}

fn build_jump_hosts(mut jump_host: Option<String>, mut user_identity: String) -> Result<Vec<JumpHostOptions>, String> {
    let mut jump_hosts = Vec::<JumpHostOptions>::new();
    while let Some(current) = jump_host.take() {
        if let Some(host_conf) = get_default_ssh_config_host(&current) {
            user_identity = expand_user_home(&host_conf.identity_file);
            jump_hosts.push(JumpHostOptions {
                username: host_conf.user,
                host: host_conf.host_name,
                port: host_conf.port,
                identity: PathBuf::from(user_identity.clone()),
                password: None,
            });
            if host_conf.proxy_jump.is_empty() {
                break;
            }
            jump_host = Some(host_conf.proxy_jump);
            continue;
        }
        let hop = parse_ssh_url(&current)?;
        jump_hosts.push(JumpHostOptions {
            username: hop.username,
            host: hop.host.trim_matches(&['[', ']'][..]).to_string(),
            port: hop.port,
            identity: PathBuf::from(user_identity.clone()),
            password: None,
        });
    }
    Ok(jump_hosts)
}

fn jump_host_options_from_conf(conf: &[JumpHostConf]) -> Result<Vec<JumpHostOptions>, String> {
    let mut jump_hosts = Vec::new();
    for item in conf {
        let parsed = parse_ssh_url(&item.uri)?;
        let identity = if item.identity.is_empty() {
            format!("{}/.ssh/id_rsa", current_home_dir())
        } else {
            expand_user_home(&item.identity)
        };
        jump_hosts.push(JumpHostOptions {
            username: parsed.username,
            host: parsed.host.trim_matches(&['[', ']'][..]).to_string(),
            port: parsed.port,
            identity: PathBuf::from(identity),
            password: if item.password.is_empty() {
                None
            } else {
                Some(item.password.clone())
            },
        });
    }
    Ok(jump_hosts)
}
