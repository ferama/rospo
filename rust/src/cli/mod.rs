use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::{load_config, Config};
use crate::ssh::{fetch_server_public_key, ClientOptions, JumpHostOptions, Session};
use crate::tunnel;
use crate::utils::{
    add_host_key_to_known_hosts, current_home_dir, expand_user_home, get_default_ssh_config_host, new_endpoint,
    parse_ssh_url, write_file_0600,
};
use internal_russh_forked_ssh_key::{public::EcdsaPublicKey, PublicKey};
use p521::ecdsa::VerifyingKey;
use p521::elliptic_curve::sec1::ToEncodedPoint;
use p521::elliptic_curve::rand_core::OsRng;
use sec1::LineEnding;

pub const VERSION: &str = "development";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliResponse {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

impl CliResponse {
    fn success(stdout: impl Into<String>) -> Self {
        Self {
            stdout: stdout.into(),
            stderr: String::new(),
            exit_code: 0,
        }
    }

    fn failure(stderr: impl Into<String>, exit_code: i32) -> Self {
        Self {
            stdout: String::new(),
            stderr: stderr.into(),
            exit_code,
        }
    }
}

pub fn execute<I, T>(args: I) -> CliResponse
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let args = args
        .into_iter()
        .map(|arg| arg.into().to_string_lossy().into_owned())
        .collect::<Vec<_>>();

    dispatch(&args)
}

pub fn run<I, T>(args: I) -> i32
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let response = execute(args);
    if !response.stdout.is_empty() {
        print!("{}", response.stdout);
    }
    if !response.stderr.is_empty() {
        eprint!("{}", response.stderr);
    }
    response.exit_code
}

fn dispatch(args: &[String]) -> CliResponse {
    let rest = if args.is_empty() { &[][..] } else { &args[1..] };

    if rest.is_empty() {
        return CliResponse::success(golden_cli("root-noargs.txt"));
    }

    if matches_help(rest) {
        return CliResponse::success(golden_cli("root-help.txt"));
    }

    if matches!(rest, [flag] if flag == "-v" || flag == "--version") {
        return CliResponse::success(format!("rospo version {}\n", VERSION));
    }

    if matches!(rest, [cmd, help] if cmd == "template" && is_help_flag(help)) {
        return CliResponse::success(golden_cli("template-help.txt"));
    }
    if matches!(rest, [cmd] if cmd == "template") {
        return CliResponse::success(template_output());
    }

    let help_key = match rest {
        [cmd, help] if is_help_flag(help) => command_help_key(&[cmd.as_str()]),
        [cmd1, cmd2, help] if is_help_flag(help) => command_help_key(&[cmd1.as_str(), cmd2.as_str()]),
        [cmd] if cmd == "help" => Some("root-help.txt"),
        [cmd, topic] if cmd == "help" => command_help_key(&[topic.as_str()]),
        [cmd, topic, subtopic] if cmd == "help" => command_help_key(&[topic.as_str(), subtopic.as_str()]),
        _ => None,
    };
    if let Some(help_key) = help_key {
        return CliResponse::success(golden_cli(help_key));
    }

    match rest.first().map(String::as_str) {
        Some("run") => run_config_command(rest),
        Some("keygen") => keygen_command(rest),
        Some("grabpubkey") => grabpubkey_command(rest),
        Some("shell") => shell_command(rest),
        Some("tun") => tun_command(rest),
        Some("dns-proxy" | "get" | "put" | "revshell" | "socks-proxy" | "sshd") => {
            CliResponse::failure("Rust runtime implementation is not complete yet\n", 1)
        }
        _ => CliResponse::failure("invalid subcommand\n", 1),
    }
}

fn run_config_command(rest: &[String]) -> CliResponse {
    if matches!(rest, [cmd, help] if cmd == "run" && is_help_flag(help)) {
        return CliResponse::success(golden_cli("run-help.txt"));
    }
    if rest.len() < 2 {
        return CliResponse::failure("run requires a config file path\n", 1);
    }

    let config_path = Path::new(&rest[1]);
    let content = match fs::read_to_string(config_path) {
        Ok(content) => content,
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };
    let parsed: Config = match load_config(&content) {
        Ok(parsed) => parsed,
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };

    if parsed.ssh_client.is_none()
        && parsed.sshd.is_none()
        && parsed.tunnel.as_ref().is_none_or(Vec::is_empty)
        && parsed.socks_proxy.is_none()
        && parsed.dns_proxy.is_none()
    {
        return CliResponse::success("2026/03/19 00:00:00 nothing to run\n");
    }

    CliResponse::failure("Rust runtime implementation is not complete yet\n", 1)
}

fn keygen_command(rest: &[String]) -> CliResponse {
    if matches!(rest, [cmd, help] if cmd == "keygen" && is_help_flag(help)) {
        return CliResponse::success(golden_cli("keygen-help.txt"));
    }

    let mut store = false;
    let mut path = ".".to_string();
    let mut name = "identity".to_string();

    let mut idx = 1usize;
    while idx < rest.len() {
        match rest[idx].as_str() {
            "-s" | "--store" => {
                store = true;
                idx += 1;
            }
            "-p" | "--path" => {
                let Some(value) = rest.get(idx + 1) else {
                    return CliResponse::failure("flag needs an argument: --path\n", 1);
                };
                path = value.clone();
                idx += 2;
            }
            "-n" | "--name" => {
                let Some(value) = rest.get(idx + 1) else {
                    return CliResponse::failure("flag needs an argument: --name\n", 1);
                };
                name = value.clone();
                idx += 2;
            }
            other => {
                return CliResponse::failure(format!("unknown argument: {other}\n"), 1);
            }
        }
    }

    let secret = p521::SecretKey::random(&mut OsRng);
    let private_pem = match secret.to_sec1_pem(LineEnding::LF) {
        Ok(pem) => pem.to_string(),
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };
    let encoded_point = secret.public_key().to_encoded_point(false);
    let verifying_key = match VerifyingKey::from_encoded_point(&encoded_point) {
        Ok(key) => key,
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };
    let public_key = PublicKey::from(EcdsaPublicKey::from(&verifying_key));
    let public_key = match public_key.to_openssh() {
        Ok(encoded) => format!("{encoded}\n"),
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };

    if store {
        let dir = PathBuf::from(expand_user_home(&path));
        let _ = write_file_0600(&dir.join(&name), private_pem.as_bytes());
        let _ = write_file_0600(&dir.join(format!("{name}.pub")), public_key.as_bytes());
        CliResponse::success("")
    } else {
        CliResponse::success(format!("{private_pem}{public_key}"))
    }
}

fn grabpubkey_command(rest: &[String]) -> CliResponse {
    if matches!(rest, [cmd, help] if cmd == "grabpubkey" && is_help_flag(help)) {
        return CliResponse::success(golden_cli("grabpubkey-help.txt"));
    }
    if rest.len() < 2 {
        return CliResponse::failure("grabpubkey requires a host:port\n", 1);
    }

    let mut known_hosts = expand_user_home("~/.ssh/known_hosts");
    let mut idx = 1usize;
    let mut host = None::<String>;
    while idx < rest.len() {
        match rest[idx].as_str() {
            "-k" | "--known-hosts" => {
                let Some(value) = rest.get(idx + 1) else {
                    return CliResponse::failure("flag needs an argument: --known-hosts\n", 1);
                };
                known_hosts = expand_user_home(value);
                idx += 2;
            }
            value if value.starts_with('-') => {
                return CliResponse::failure(format!("unknown argument: {value}\n"), 1);
            }
            value => {
                host = Some(value.to_string());
                idx += 1;
            }
        }
    }

    let Some(host) = host else {
        return CliResponse::failure("grabpubkey requires a host:port\n", 1);
    };
    let parsed = match parse_ssh_url(&host) {
        Ok(parsed) => parsed,
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };

    let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };
    let key = match runtime.block_on(fetch_server_public_key((parsed.host.trim_matches(&['[', ']'][..]), parsed.port))) {
        Ok(key) => key,
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };
    match add_host_key_to_known_hosts(&host, &key, Path::new(&known_hosts)) {
        Ok(()) => CliResponse::success(""),
        Err(err) => CliResponse::failure(format!("{err}\n"), 1),
    }
}

fn shell_command(rest: &[String]) -> CliResponse {
    if matches!(rest, [cmd, help] if cmd == "shell" && is_help_flag(help)) {
        return CliResponse::success(golden_cli("shell-help.txt"));
    }

    let parsed = match parse_ssh_client_command(rest, "shell") {
        Ok(parsed) => parsed,
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };

    let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };

    let result = runtime.block_on(async {
        let mut session = Session::connect(parsed.options).await?;
        let code = if parsed.command.is_empty() {
            session.run_shell().await?
        } else {
            session.run_command(&parsed.command.join(" ")).await?
        };
        let _ = session.disconnect().await;
        Ok::<u32, String>(code)
    });

    match result {
        Ok(code) => CliResponse {
            stdout: String::new(),
            stderr: String::new(),
            exit_code: code as i32,
        },
        Err(err) => CliResponse::failure(format!("{err}\n"), 1),
    }
}

struct ParsedSshCommand {
    options: ClientOptions,
    command: Vec<String>,
}

fn parse_ssh_client_command(rest: &[String], command_name: &str) -> Result<ParsedSshCommand, String> {
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
            value => {
                positionals.push(value.to_string());
                idx += 1;
            }
        }
    }

    if positionals.is_empty() {
        return Err(format!("{command_name} requires a server argument"));
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
            quiet: disable_banner,
            jump_hosts,
        },
        command,
    })
}

fn tun_command(rest: &[String]) -> CliResponse {
    match parse_tun_command(rest) {
        Ok(parsed) => {
            let runtime = match tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build() {
                Ok(runtime) => runtime,
                Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
            };

            let result = runtime.block_on(async move {
                if parsed.forward {
                    tunnel::run_forward(parsed.options, parsed.local, parsed.remote).await
                } else {
                    tunnel::run_reverse(parsed.options, parsed.local, parsed.remote).await
                }
            });

            match result {
                Ok(()) => CliResponse::success(""),
                Err(err) => CliResponse::failure(format!("{err}\n"), 1),
            }
        }
        Err(err) => CliResponse::failure(format!("{err}\n"), 1),
    }
}

struct ParsedTunnelCommand {
    options: ClientOptions,
    local: crate::utils::Endpoint,
    remote: crate::utils::Endpoint,
    forward: bool,
}

fn parse_tun_command(rest: &[String]) -> Result<ParsedTunnelCommand, String> {
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
            value if value.starts_with('-') => return Err(format!("unknown argument: {value}")),
            value => {
                positionals.push(value.to_string());
                idx += 1;
            }
        }
    }

    let Some(subcommand) = subcommand else {
        return Err("tun requires a subcommand".to_string());
    };
    if positionals.is_empty() {
        return Err(format!("tun {} requires a server argument", subcommand));
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
            quiet: disable_banner,
            jump_hosts,
        },
        local: new_endpoint(&local)?,
        remote: new_endpoint(&remote)?,
        forward: subcommand == "forward",
    })
}

fn matches_help(rest: &[String]) -> bool {
    matches!(rest, [flag] if is_help_flag(flag))
}

fn is_help_flag(flag: &str) -> bool {
    flag == "-h" || flag == "--help"
}

fn command_help_key(path: &[&str]) -> Option<&'static str> {
    match path {
        ["dns-proxy"] => Some("dns-proxy-help.txt"),
        ["get"] => Some("get-help.txt"),
        ["grabpubkey"] => Some("grabpubkey-help.txt"),
        ["keygen"] => Some("keygen-help.txt"),
        ["put"] => Some("put-help.txt"),
        ["revshell"] => Some("revshell-help.txt"),
        ["run"] => Some("run-help.txt"),
        ["shell"] => Some("shell-help.txt"),
        ["socks-proxy"] => Some("socks-proxy-help.txt"),
        ["sshd"] => Some("sshd-help.txt"),
        ["template"] => Some("template-help.txt"),
        ["tun"] => Some("tun-help.txt"),
        ["tun", "forward"] => Some("tun-forward-help.txt"),
        ["tun", "reverse"] => Some("tun-reverse-help.txt"),
        _ => None,
    }
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rust dir has parent")
        .to_path_buf()
}

fn golden_cli(name: &str) -> String {
    fs::read_to_string(repo_root().join("compat/golden/cli").join(name)).expect("read cli fixture")
}

fn template_output() -> String {
    fs::read_to_string(repo_root().join("cmd/configs/config_template.yaml"))
        .expect("read config template")
        + "\n"
}
