use std::path::PathBuf;

use crate::config::{JumpHostConf, SshClientConf};
use crate::logging;
use crate::sftp;
use crate::ssh::{ClientOptions, JumpHostOptions};
use crate::sshd::ServerOptions;
use crate::utils::{current_home_dir, expand_user_home, get_default_ssh_config_host, new_endpoint, parse_ssh_url, Endpoint};

pub(crate) struct ParsedSshCommand {
    pub(crate) options: ClientOptions,
    pub(crate) command: Vec<String>,
}

pub(crate) struct ParsedGetCommand {
    pub(crate) options: ClientOptions,
    pub(crate) remote: String,
    pub(crate) local: String,
    pub(crate) recursive: bool,
    pub(crate) max_workers: usize,
    pub(crate) concurrent_transfers: usize,
}

pub(crate) struct ParsedPutCommand {
    pub(crate) options: ClientOptions,
    pub(crate) local: String,
    pub(crate) remote: String,
    pub(crate) recursive: bool,
    pub(crate) max_workers: usize,
    pub(crate) concurrent_transfers: usize,
}

pub(crate) struct ParsedTunnelCommand {
    pub(crate) options: ClientOptions,
    pub(crate) local: Endpoint,
    pub(crate) remote: Endpoint,
    pub(crate) forward: bool,
}

pub(crate) fn parse_ssh_client_command(rest: &[String], _command_name: &str) -> Result<ParsedSshCommand, String> {
    let default_identity = format!("{}/.ssh/id_rsa", current_home_dir());
    let default_known_hosts = format!("{}/.ssh/known_hosts", current_home_dir());
    let mut disable_banner = false;
    let mut insecure = false;
    let mut user_identity = default_identity;
    let mut known_hosts = default_known_hosts;
    let mut password = None::<String>;
    let mut jump_host = None::<String>;
    let mut user_identity_changed = false;
    let mut known_hosts_changed = false;
    let mut insecure_changed = false;
    let mut jump_host_changed = false;
    let mut positionals = Vec::<String>::new();

    let mut idx = 1usize;
    while idx < rest.len() {
        match rest[idx].as_str() {
            "-b" | "--disable-banner" => {
                disable_banner = true;
                idx += 1;
            }
            "-i" | "--insecure" => {
                insecure = true;
                insecure_changed = true;
                idx += 1;
            }
            "-j" | "--jump-host" => {
                let Some(value) = rest.get(idx + 1) else { return Err("flag needs an argument: --jump-host".to_string()); };
                jump_host = Some(value.clone());
                jump_host_changed = true;
                idx += 2;
            }
            "-s" | "--user-identity" => {
                let Some(value) = rest.get(idx + 1) else { return Err("flag needs an argument: --user-identity".to_string()); };
                user_identity = expand_user_home(value);
                user_identity_changed = true;
                idx += 2;
            }
            "-k" | "--known-hosts" => {
                let Some(value) = rest.get(idx + 1) else { return Err("flag needs an argument: --known-hosts".to_string()); };
                known_hosts = expand_user_home(value);
                known_hosts_changed = true;
                idx += 2;
            }
            "-p" | "--password" => {
                let Some(value) = rest.get(idx + 1) else { return Err("flag needs an argument: --password".to_string()); };
                password = Some(value.clone());
                idx += 2;
            }
            value if value.starts_with('-') => return Err(format!("unknown flag: {value}")),
            value => {
                positionals.push(value.to_string());
                idx += 1;
            }
        }
    }

    if positionals.is_empty() {
        return Err(format!("requires at least 1 arg(s), only received {}", positionals.len()));
    }
    let server = positionals.remove(0);

    if let Some(host_conf) = get_default_ssh_config_host(&server) {
        if !user_identity_changed {
            user_identity = expand_user_home(&host_conf.identity_file);
        }
        if !known_hosts_changed {
            known_hosts = expand_user_home(&host_conf.user_known_hosts_file);
        }
        if !jump_host_changed && !host_conf.proxy_jump.is_empty() {
            jump_host = Some(host_conf.proxy_jump.clone());
        }
        if !insecure_changed {
            insecure = !host_conf.strict_host_key_checking;
        }
        let resolved_server = format!("{}@{}:{}", host_conf.user, host_conf.host_name, host_conf.port);
        return build_parsed_command(
            &resolved_server,
            jump_host,
            user_identity,
            known_hosts,
            password,
            insecure,
            disable_banner,
            positionals,
        );
    }

    build_parsed_command(
        &server,
        jump_host,
        user_identity,
        known_hosts,
        password,
        insecure,
        disable_banner,
        positionals,
    )
}

fn build_parsed_command(
    server: &str,
    mut jump_host: Option<String>,
    mut user_identity: String,
    known_hosts: String,
    password: Option<String>,
    insecure: bool,
    disable_banner: bool,
    command: Vec<String>,
) -> Result<ParsedSshCommand, String> {
    let parsed = parse_ssh_url(server)?;
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

    Ok(ParsedSshCommand {
        options: ClientOptions {
            username: parsed.username,
            host: parsed.host.trim_matches(&['[', ']'][..]).to_string(),
            port: parsed.port,
            identity: PathBuf::from(user_identity),
            known_hosts: PathBuf::from(known_hosts),
            password,
            insecure,
            quiet: disable_banner || logging::is_quiet(),
            jump_hosts,
        },
        command,
    })
}

pub(crate) fn parse_get_command(rest: &[String]) -> Result<ParsedGetCommand, String> {
    let (options, positionals, recursive, max_workers, concurrent_transfers) =
        parse_ssh_flags_and_positionals(rest, "get")?;
    if positionals.len() < 2 {
        return Err("get requires a server and remote path".to_string());
    }
    let remote = positionals[1].clone();
    let local = positionals.get(2).cloned().unwrap_or_default();
    Ok(ParsedGetCommand {
        options,
        remote,
        local,
        recursive,
        max_workers,
        concurrent_transfers,
    })
}

pub(crate) fn parse_put_command(rest: &[String]) -> Result<ParsedPutCommand, String> {
    let (options, positionals, recursive, max_workers, concurrent_transfers) =
        parse_ssh_flags_and_positionals(rest, "put")?;
    if positionals.len() < 2 {
        return Err("put requires a server and local path".to_string());
    }
    let local = positionals[1].clone();
    let remote = positionals.get(2).cloned().unwrap_or_default();
    Ok(ParsedPutCommand {
        options,
        local,
        remote,
        recursive,
        max_workers,
        concurrent_transfers,
    })
}

fn parse_ssh_flags_and_positionals(
    rest: &[String],
    command_name: &str,
) -> Result<(ClientOptions, Vec<String>, bool, usize, usize), String> {
    let default_identity = format!("{}/.ssh/id_rsa", current_home_dir());
    let default_known_hosts = format!("{}/.ssh/known_hosts", current_home_dir());
    let mut max_workers = sftp::DEFAULT_MAX_WORKERS;
    let mut concurrent_transfers = if command_name == "get" {
        sftp::DEFAULT_CONCURRENT_DOWNLOADS
    } else {
        sftp::DEFAULT_CONCURRENT_UPLOADS
    };
    let mut disable_banner = false;
    let mut insecure = false;
    let mut user_identity = default_identity;
    let mut known_hosts = default_known_hosts;
    let mut password = None::<String>;
    let mut jump_host = None::<String>;
    let mut user_identity_changed = false;
    let mut known_hosts_changed = false;
    let mut insecure_changed = false;
    let mut jump_host_changed = false;
    let mut recursive = false;
    let mut positionals = Vec::<String>::new();

    let mut idx = 1usize;
    while idx < rest.len() {
        match rest[idx].as_str() {
            "-b" | "--disable-banner" => {
                disable_banner = true;
                idx += 1;
            }
            "-i" | "--insecure" => {
                insecure = true;
                insecure_changed = true;
                idx += 1;
            }
            "-j" | "--jump-host" => {
                let Some(value) = rest.get(idx + 1) else {
                    return Err("flag needs an argument: --jump-host".to_string());
                };
                jump_host = Some(value.clone());
                jump_host_changed = true;
                idx += 2;
            }
            "-s" | "--user-identity" => {
                let Some(value) = rest.get(idx + 1) else {
                    return Err("flag needs an argument: --user-identity".to_string());
                };
                user_identity = expand_user_home(value);
                user_identity_changed = true;
                idx += 2;
            }
            "-k" | "--known-hosts" => {
                let Some(value) = rest.get(idx + 1) else {
                    return Err("flag needs an argument: --known-hosts".to_string());
                };
                known_hosts = expand_user_home(value);
                known_hosts_changed = true;
                idx += 2;
            }
            "-p" | "--password" => {
                let Some(value) = rest.get(idx + 1) else {
                    return Err("flag needs an argument: --password".to_string());
                };
                password = Some(value.clone());
                idx += 2;
            }
            "-r" | "--recursive" => {
                recursive = true;
                idx += 1;
            }
            "-w" | "--max-workers" | "-c" | "--concurrent-downloads" | "--concurrent-uploads" => {
                let Some(value) = rest.get(idx + 1) else {
                    return Err(format!("flag needs an argument: {}", rest[idx]));
                };
                let parsed = value
                    .parse::<usize>()
                    .map_err(|_| format!("invalid value for {}: {}", rest[idx], value))?;
                if parsed == 0 {
                    return Err(format!("invalid value for {}: {}", rest[idx], value));
                }
                if rest[idx] == "-w" || rest[idx] == "--max-workers" {
                    max_workers = parsed;
                } else {
                    concurrent_transfers = parsed;
                }
                idx += 2;
            }
            value if value.starts_with('-') => return Err(format!("unknown flag: {value}")),
            value => {
                positionals.push(value.to_string());
                idx += 1;
            }
        }
    }

    if positionals.len() < 2 {
        return Err(format!("requires at least 2 arg(s), only received {}", positionals.len()));
    }

    let server = positionals[0].clone();
    let parsed = if let Some(host_conf) = get_default_ssh_config_host(&server) {
        if !user_identity_changed {
            user_identity = expand_user_home(&host_conf.identity_file);
        }
        if !known_hosts_changed {
            known_hosts = expand_user_home(&host_conf.user_known_hosts_file);
        }
        if !jump_host_changed && !host_conf.proxy_jump.is_empty() {
            jump_host = Some(host_conf.proxy_jump.clone());
        }
        if !insecure_changed {
            insecure = !host_conf.strict_host_key_checking;
        }
        parse_ssh_url(&format!("{}@{}:{}", host_conf.user, host_conf.host_name, host_conf.port))?
    } else {
        parse_ssh_url(&server)?
    };

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

    Ok((
        ClientOptions {
            username: parsed.username,
            host: parsed.host.trim_matches(&['[', ']'][..]).to_string(),
            port: parsed.port,
            identity: PathBuf::from(user_identity),
            known_hosts: PathBuf::from(known_hosts),
            password,
            insecure,
            quiet: disable_banner || logging::is_quiet(),
            jump_hosts,
        },
        positionals,
        recursive,
        max_workers,
        concurrent_transfers,
    ))
}

pub(crate) fn parse_socks_proxy_command(rest: &[String]) -> Result<(ClientOptions, String), String> {
    let default_identity = format!("{}/.ssh/id_rsa", current_home_dir());
    let default_known_hosts = format!("{}/.ssh/known_hosts", current_home_dir());
    let mut disable_banner = false;
    let mut insecure = false;
    let mut user_identity = default_identity;
    let mut known_hosts = default_known_hosts;
    let mut password = None::<String>;
    let mut jump_host = None::<String>;
    let mut user_identity_changed = false;
    let mut known_hosts_changed = false;
    let mut insecure_changed = false;
    let mut jump_host_changed = false;
    let mut listen_address = "127.0.0.1:1080".to_string();
    let mut positionals = Vec::<String>::new();

    let mut idx = 1usize;
    while idx < rest.len() {
        match rest[idx].as_str() {
            "-b" | "--disable-banner" => {
                disable_banner = true;
                idx += 1;
            }
            "-i" | "--insecure" => {
                insecure = true;
                insecure_changed = true;
                idx += 1;
            }
            "-j" | "--jump-host" => {
                let Some(value) = rest.get(idx + 1) else { return Err("flag needs an argument: --jump-host".to_string()); };
                jump_host = Some(value.clone());
                jump_host_changed = true;
                idx += 2;
            }
            "-s" | "--user-identity" => {
                let Some(value) = rest.get(idx + 1) else { return Err("flag needs an argument: --user-identity".to_string()); };
                user_identity = expand_user_home(value);
                user_identity_changed = true;
                idx += 2;
            }
            "-k" | "--known-hosts" => {
                let Some(value) = rest.get(idx + 1) else { return Err("flag needs an argument: --known-hosts".to_string()); };
                known_hosts = expand_user_home(value);
                known_hosts_changed = true;
                idx += 2;
            }
            "-p" | "--password" => {
                let Some(value) = rest.get(idx + 1) else { return Err("flag needs an argument: --password".to_string()); };
                password = Some(value.clone());
                idx += 2;
            }
            "-l" | "--listen-address" => {
                let Some(value) = rest.get(idx + 1) else { return Err("flag needs an argument: --listen-address".to_string()); };
                listen_address = value.clone();
                idx += 2;
            }
            value if value.starts_with('-') => return Err(format!("unknown flag: {value}")),
            value => {
                positionals.push(value.to_string());
                idx += 1;
            }
        }
    }
    if positionals.is_empty() {
        return Err(format!("requires at least 1 arg(s), only received {}", positionals.len()));
    }
    let options = build_client_options_from_server(
        &positionals[0],
        user_identity,
        known_hosts,
        password,
        insecure,
        disable_banner,
        jump_host,
        user_identity_changed,
        known_hosts_changed,
        insecure_changed,
        jump_host_changed,
    )?;
    Ok((options, listen_address))
}

pub(crate) fn parse_dns_proxy_command(rest: &[String]) -> Result<(ClientOptions, String, String), String> {
    let default_identity = format!("{}/.ssh/id_rsa", current_home_dir());
    let default_known_hosts = format!("{}/.ssh/known_hosts", current_home_dir());
    let mut disable_banner = false;
    let mut insecure = false;
    let mut user_identity = default_identity;
    let mut known_hosts = default_known_hosts;
    let mut password = None::<String>;
    let mut jump_host = None::<String>;
    let mut user_identity_changed = false;
    let mut known_hosts_changed = false;
    let mut insecure_changed = false;
    let mut jump_host_changed = false;
    let mut listen_address = ":53".to_string();
    let mut remote_dns = "1.1.1.1:53".to_string();
    let mut positionals = Vec::<String>::new();

    let mut idx = 1usize;
    while idx < rest.len() {
        match rest[idx].as_str() {
            "-b" | "--disable-banner" => {
                disable_banner = true;
                idx += 1;
            }
            "-i" | "--insecure" => {
                insecure = true;
                insecure_changed = true;
                idx += 1;
            }
            "-j" | "--jump-host" => {
                let Some(value) = rest.get(idx + 1) else { return Err("flag needs an argument: --jump-host".to_string()); };
                jump_host = Some(value.clone());
                jump_host_changed = true;
                idx += 2;
            }
            "-s" | "--user-identity" => {
                let Some(value) = rest.get(idx + 1) else { return Err("flag needs an argument: --user-identity".to_string()); };
                user_identity = expand_user_home(value);
                user_identity_changed = true;
                idx += 2;
            }
            "-k" | "--known-hosts" => {
                let Some(value) = rest.get(idx + 1) else { return Err("flag needs an argument: --known-hosts".to_string()); };
                known_hosts = expand_user_home(value);
                known_hosts_changed = true;
                idx += 2;
            }
            "-p" | "--password" => {
                let Some(value) = rest.get(idx + 1) else { return Err("flag needs an argument: --password".to_string()); };
                password = Some(value.clone());
                idx += 2;
            }
            "-l" | "--listen-address" => {
                let Some(value) = rest.get(idx + 1) else { return Err("flag needs an argument: --listen-address".to_string()); };
                listen_address = value.clone();
                idx += 2;
            }
            "-d" | "--remote-dns-server" => {
                let Some(value) = rest.get(idx + 1) else { return Err("flag needs an argument: --remote-dns-server".to_string()); };
                remote_dns = value.clone();
                idx += 2;
            }
            value if value.starts_with('-') => return Err(format!("unknown flag: {value}")),
            value => {
                positionals.push(value.to_string());
                idx += 1;
            }
        }
    }
    if positionals.is_empty() {
        return Err(format!("requires at least 1 arg(s), only received {}", positionals.len()));
    }
    let options = build_client_options_from_server(
        &positionals[0],
        user_identity,
        known_hosts,
        password,
        insecure,
        disable_banner,
        jump_host,
        user_identity_changed,
        known_hosts_changed,
        insecure_changed,
        jump_host_changed,
    )?;
    Ok((options, listen_address, remote_dns))
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
            quiet: disable_banner || logging::is_quiet(),
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
        quiet: disable_banner || logging::is_quiet(),
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
        password: if conf.password.is_empty() { None } else { Some(conf.password.clone()) },
        insecure: conf.insecure,
        quiet: conf.quiet || logging::is_quiet(),
        jump_hosts: jump_host_options_from_conf(&conf.jump_hosts)?,
    })
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
            password: if item.password.is_empty() { None } else { Some(item.password.clone()) },
        });
    }
    Ok(jump_hosts)
}

pub(crate) fn parse_tun_command(rest: &[String]) -> Result<ParsedTunnelCommand, String> {
    let default_identity = format!("{}/.ssh/id_rsa", current_home_dir());
    let default_known_hosts = format!("{}/.ssh/known_hosts", current_home_dir());
    let mut disable_banner = false;
    let mut insecure = false;
    let mut user_identity = default_identity;
    let mut known_hosts = default_known_hosts;
    let mut password = None::<String>;
    let mut jump_host = None::<String>;
    let mut local = "127.0.0.1:2222".to_string();
    let mut remote = "127.0.0.1:2222".to_string();
    let mut user_identity_changed = false;
    let mut known_hosts_changed = false;
    let mut insecure_changed = false;
    let mut jump_host_changed = false;
    let mut subcommand = None::<String>;
    let mut positionals = Vec::<String>::new();

    let mut idx = 1usize;
    while idx < rest.len() {
        match rest[idx].as_str() {
            "forward" | "reverse" if subcommand.is_none() => {
                subcommand = Some(rest[idx].clone());
                idx += 1;
            }
            "-b" | "--disable-banner" => {
                disable_banner = true;
                idx += 1;
            }
            "-i" | "--insecure" => {
                insecure = true;
                insecure_changed = true;
                idx += 1;
            }
            "-j" | "--jump-host" => {
                let Some(value) = rest.get(idx + 1) else {
                    return Err("flag needs an argument: --jump-host".to_string());
                };
                jump_host = Some(value.clone());
                jump_host_changed = true;
                idx += 2;
            }
            "-s" | "--user-identity" => {
                let Some(value) = rest.get(idx + 1) else {
                    return Err("flag needs an argument: --user-identity".to_string());
                };
                user_identity = expand_user_home(value);
                user_identity_changed = true;
                idx += 2;
            }
            "-k" | "--known-hosts" => {
                let Some(value) = rest.get(idx + 1) else {
                    return Err("flag needs an argument: --known-hosts".to_string());
                };
                known_hosts = expand_user_home(value);
                known_hosts_changed = true;
                idx += 2;
            }
            "-p" | "--password" => {
                let Some(value) = rest.get(idx + 1) else {
                    return Err("flag needs an argument: --password".to_string());
                };
                password = Some(value.clone());
                idx += 2;
            }
            "-l" | "--local" => {
                let Some(value) = rest.get(idx + 1) else {
                    return Err("flag needs an argument: --local".to_string());
                };
                local = value.clone();
                idx += 2;
            }
            "-r" | "--remote" => {
                let Some(value) = rest.get(idx + 1) else {
                    return Err("flag needs an argument: --remote".to_string());
                };
                remote = value.clone();
                idx += 2;
            }
            value if value.starts_with('-') => return Err(format!("unknown flag: {value}")),
            value => {
                positionals.push(value.to_string());
                idx += 1;
            }
        }
    }

    let Some(subcommand) = subcommand else {
        return Err("requires at least 1 arg(s), only received 0".to_string());
    };
    if positionals.is_empty() {
        return Err(format!("requires at least 1 arg(s), only received {}", positionals.len()));
    }

    let server = positionals.remove(0);
    let parsed = if let Some(host_conf) = get_default_ssh_config_host(&server) {
        if !user_identity_changed {
            user_identity = expand_user_home(&host_conf.identity_file);
        }
        if !known_hosts_changed {
            known_hosts = expand_user_home(&host_conf.user_known_hosts_file);
        }
        if !jump_host_changed && !host_conf.proxy_jump.is_empty() {
            jump_host = Some(host_conf.proxy_jump.clone());
        }
        if !insecure_changed {
            insecure = !host_conf.strict_host_key_checking;
        }
        parse_ssh_url(&format!("{}@{}:{}", host_conf.user, host_conf.host_name, host_conf.port))?
    } else {
        parse_ssh_url(&server)?
    };

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

    Ok(ParsedTunnelCommand {
        options: ClientOptions {
            username: parsed.username,
            host: parsed.host.trim_matches(&['[', ']'][..]).to_string(),
            port: parsed.port,
            identity: PathBuf::from(user_identity),
            known_hosts: PathBuf::from(known_hosts),
            password,
            insecure,
            quiet: disable_banner || logging::is_quiet(),
            jump_hosts,
        },
        local: new_endpoint(&local)?,
        remote: new_endpoint(&remote)?,
        forward: subcommand == "forward",
    })
}

pub(crate) fn parse_sshd_command(rest: &[String]) -> Result<ServerOptions, String> {
    let mut authorized_keys = vec!["./authorized_keys".to_string()];
    let mut listen_address = ":2222".to_string();
    let mut server_key = "./server_key".to_string();
    let mut disable_auth = false;
    let mut disable_shell = false;
    let mut authorized_password = String::new();

    let mut idx = 1usize;
    while idx < rest.len() {
        match rest[idx].as_str() {
            "-K" | "--sshd-authorized-keys" => {
                let Some(value) = rest.get(idx + 1) else {
                    return Err("flag needs an argument: --sshd-authorized-keys".to_string());
                };
                authorized_keys = vec![value.clone()];
                idx += 2;
            }
            "-P" | "--sshd-listen-address" => {
                let Some(value) = rest.get(idx + 1) else {
                    return Err("flag needs an argument: --sshd-listen-address".to_string());
                };
                listen_address = value.clone();
                idx += 2;
            }
            "-I" | "--sshd-key" => {
                let Some(value) = rest.get(idx + 1) else {
                    return Err("flag needs an argument: --sshd-key".to_string());
                };
                server_key = value.clone();
                idx += 2;
            }
            "-T" | "--disable-auth" => {
                disable_auth = true;
                idx += 1;
            }
            "-D" | "--disable-shell" => {
                disable_shell = true;
                idx += 1;
            }
            "-A" | "--sshd-authorized-password" => {
                let Some(value) = rest.get(idx + 1) else {
                    return Err("flag needs an argument: --sshd-authorized-password".to_string());
                };
                authorized_password = value.clone();
                idx += 2;
            }
            value if value.starts_with('-') => return Err(format!("unknown flag: {value}")),
            value => return Err(format!("unexpected argument: {value}")),
        }
    }

    Ok(ServerOptions {
        server_key,
        authorized_keys,
        authorized_password,
        listen_address,
        disable_shell,
        disable_banner: false,
        disable_auth,
        disable_sftp_subsystem: false,
        disable_tunnelling: false,
        shell_executable: String::new(),
    })
}

pub(crate) fn parse_revshell_command(
    rest: &[String],
) -> Result<(ClientOptions, ServerOptions, Endpoint, Endpoint), String> {
    let default_identity = format!("{}/.ssh/id_rsa", current_home_dir());
    let default_known_hosts = format!("{}/.ssh/known_hosts", current_home_dir());
    let mut disable_banner = false;
    let mut insecure = false;
    let mut user_identity = default_identity;
    let mut known_hosts = default_known_hosts;
    let mut password = None::<String>;
    let mut jump_host = None::<String>;
    let mut user_identity_changed = false;
    let mut known_hosts_changed = false;
    let mut insecure_changed = false;
    let mut jump_host_changed = false;

    let mut authorized_keys = vec!["./authorized_keys".to_string()];
    let mut listen_address = ":2222".to_string();
    let mut server_key = "./server_key".to_string();
    let mut disable_auth = false;
    let mut authorized_password = String::new();
    let mut remote = "127.0.0.1:2222".to_string();
    let mut positionals = Vec::<String>::new();

    let mut idx = 1usize;
    while idx < rest.len() {
        match rest[idx].as_str() {
            "-b" | "--disable-banner" => {
                disable_banner = true;
                idx += 1;
            }
            "-i" | "--insecure" => {
                insecure = true;
                insecure_changed = true;
                idx += 1;
            }
            "-j" | "--jump-host" => {
                let Some(value) = rest.get(idx + 1) else {
                    return Err("flag needs an argument: --jump-host".to_string());
                };
                jump_host = Some(value.clone());
                jump_host_changed = true;
                idx += 2;
            }
            "-s" | "--user-identity" => {
                let Some(value) = rest.get(idx + 1) else {
                    return Err("flag needs an argument: --user-identity".to_string());
                };
                user_identity = expand_user_home(value);
                user_identity_changed = true;
                idx += 2;
            }
            "-k" | "--known-hosts" => {
                let Some(value) = rest.get(idx + 1) else {
                    return Err("flag needs an argument: --known-hosts".to_string());
                };
                known_hosts = expand_user_home(value);
                known_hosts_changed = true;
                idx += 2;
            }
            "-p" | "--password" => {
                let Some(value) = rest.get(idx + 1) else {
                    return Err("flag needs an argument: --password".to_string());
                };
                password = Some(value.clone());
                idx += 2;
            }
            "-r" | "--remote" => {
                let Some(value) = rest.get(idx + 1) else {
                    return Err("flag needs an argument: --remote".to_string());
                };
                remote = value.clone();
                idx += 2;
            }
            "-K" | "--sshd-authorized-keys" => {
                let Some(value) = rest.get(idx + 1) else {
                    return Err("flag needs an argument: --sshd-authorized-keys".to_string());
                };
                authorized_keys = vec![value.clone()];
                idx += 2;
            }
            "-P" | "--sshd-listen-address" => {
                let Some(value) = rest.get(idx + 1) else {
                    return Err("flag needs an argument: --sshd-listen-address".to_string());
                };
                listen_address = value.clone();
                idx += 2;
            }
            "-I" | "--sshd-key" => {
                let Some(value) = rest.get(idx + 1) else {
                    return Err("flag needs an argument: --sshd-key".to_string());
                };
                server_key = value.clone();
                idx += 2;
            }
            "-T" | "--disable-auth" => {
                disable_auth = true;
                idx += 1;
            }
            "-A" | "--sshd-authorized-password" => {
                let Some(value) = rest.get(idx + 1) else {
                    return Err("flag needs an argument: --sshd-authorized-password".to_string());
                };
                authorized_password = value.clone();
                idx += 2;
            }
            value if value.starts_with('-') => return Err(format!("unknown flag: {value}")),
            value => {
                positionals.push(value.to_string());
                idx += 1;
            }
        }
    }

    if positionals.is_empty() {
        return Err(format!("requires at least 1 arg(s), only received {}", positionals.len()));
    }

    let client_options = build_client_options_from_server(
        &positionals[0],
        user_identity,
        known_hosts,
        password,
        insecure,
        disable_banner,
        jump_host,
        user_identity_changed,
        known_hosts_changed,
        insecure_changed,
        jump_host_changed,
    )?;

    let server_options = ServerOptions {
        server_key,
        authorized_keys,
        authorized_password,
        listen_address: listen_address.clone(),
        disable_shell: false,
        disable_banner: false,
        disable_auth,
        disable_sftp_subsystem: false,
        disable_tunnelling: false,
        shell_executable: String::new(),
    };

    Ok((
        client_options,
        server_options,
        new_endpoint(&listen_address)?,
        new_endpoint(&remote)?,
    ))
}
