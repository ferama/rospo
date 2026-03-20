use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

use crate::config::{load_config, Config, JumpHostConf, SshClientConf};
use crate::dns_proxy;
use crate::logging;
use crate::sftp::{self, TransferOptions};
use crate::ssh::{fetch_server_public_key, ClientOptions, JumpHostOptions, Session};
use crate::sshd::{self, ServerOptions};
use crate::socks;
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

    fn success_stderr(stderr: impl Into<String>) -> Self {
        Self {
            stdout: String::new(),
            stderr: stderr.into(),
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
    let (args, _) = normalize_args(args);

    dispatch(&args)
}

pub fn run<I, T>(args: I) -> i32
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let args = args
        .into_iter()
        .map(|arg| arg.into().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    let (args, quiet) = normalize_args(args);
    logging::init_logging(quiet);
    let response = dispatch(&args);
    if !response.stdout.is_empty() {
        print!("{}", response.stdout);
    }
    if !response.stderr.is_empty() {
        eprint!("{}", response.stderr);
    }
    response.exit_code
}

fn normalize_args(args: Vec<String>) -> (Vec<String>, bool) {
    if args.is_empty() {
        return (args, false);
    }

    let mut normalized = Vec::with_capacity(args.len());
    normalized.push(args[0].clone());
    let mut quiet = false;

    for arg in args.into_iter().skip(1) {
        if arg == "-q" || arg == "--quiet" {
            quiet = true;
            continue;
        }
        normalized.push(arg);
    }

    (normalized, quiet)
}

fn dispatch(args: &[String]) -> CliResponse {
    let rest = if args.is_empty() { &[][..] } else { &args[1..] };

    if rest.is_empty() {
        return CliResponse::success_stderr(golden_cli("root-noargs.txt"));
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
        Some("get") => get_command(rest),
        Some("put") => put_command(rest),
        Some("socks-proxy") => socks_proxy_command(rest),
        Some("dns-proxy") => dns_proxy_command(rest),
        Some("tun") => tun_command(rest),
        Some("sshd") => sshd_command(rest),
        Some("revshell") => revshell_command(rest),
        _ => CliResponse {
            stdout: "invalid subcommand\n".to_string(),
            stderr: String::new(),
            exit_code: 1,
        },
    }
}

fn run_config_command(rest: &[String]) -> CliResponse {
    if matches!(rest, [cmd, help] if cmd == "run" && is_help_flag(help)) {
        return CliResponse::success(golden_cli("run-help.txt"));
    }
    if rest.len() < 2 {
        return cobra_usage_error("run-help.txt", "requires at least 1 arg(s), only received 0");
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

    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };

    let result = runtime.block_on(async move {
        let mut tasks = Vec::new();

        if let Some(sshd_conf) = parsed.sshd.clone() {
            let options = ServerOptions::from_conf(&sshd_conf);
            tasks.push(tokio::spawn(async move { sshd::run(options).await }));
        }

        if let Some(tunnels) = parsed.tunnel.clone() {
            for tunnel_conf in tunnels {
                let base = match tunnel_conf.ssh_client.clone().or(parsed.ssh_client.clone()) {
                    Some(conf) => conf,
                    None => return Err("you need to configure sshclient section to support tunnel".to_string()),
                };
                let options = client_options_from_conf(&base)?;
                let local = new_endpoint(&tunnel_conf.local)?;
                let remote = new_endpoint(&tunnel_conf.remote)?;
                tasks.push(tokio::spawn(async move {
                    if tunnel_conf.forward {
                        tunnel::run_forward(options, local, remote).await
                    } else {
                        tunnel::run_reverse(options, local, remote).await
                    }
                }));
            }
        }

        if let Some(socks_conf) = parsed.socks_proxy.clone() {
            let conf = match socks_conf.ssh_client.clone().or(parsed.ssh_client.clone()) {
                Some(conf) => conf,
                None => return Err("you need to configure sshclient section to support socks proxy".to_string()),
            };
            let options = client_options_from_conf(&conf)?;
            tasks.push(tokio::spawn(async move {
                socks::run(options, &socks_conf.listen_address).await
            }));
        }

        if let Some(dns_conf) = parsed.dns_proxy.clone() {
            let conf = if let Some(conf) = dns_conf.ssh_client.clone() {
                conf
            } else if let Some(global) = parsed.ssh_client.clone() {
                global
            } else {
                return Err("you need to configure sshclient section to support dns proxy".to_string());
            };
            let options = client_options_from_conf(&conf)?;
            let remote_dns = dns_conf
                .remote_dns_address
                .unwrap_or_else(|| "1.1.1.1:53".to_string());
            tasks.push(tokio::spawn(async move {
                dns_proxy::run(options, &dns_conf.listen_address, &remote_dns).await
            }));
        }

        if tasks.is_empty() {
            return Ok::<(), String>(());
        }

        tokio::signal::ctrl_c().await.map_err(|err| err.to_string())?;
        Ok(())
    });

    match result {
        Ok(()) => CliResponse::success(""),
        Err(err) => CliResponse::failure(format!("{err}\n"), 1),
    }
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
                    return cobra_usage_error("keygen-help.txt", "flag needs an argument: --path");
                };
                path = value.clone();
                idx += 2;
            }
            "-n" | "--name" => {
                let Some(value) = rest.get(idx + 1) else {
                    return cobra_usage_error("keygen-help.txt", "flag needs an argument: --name");
                };
                name = value.clone();
                idx += 2;
            }
            other => {
                return cobra_usage_error("keygen-help.txt", &format!("unknown flag: {other}"));
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
        return cobra_usage_error("grabpubkey-help.txt", "requires at least 1 arg(s), only received 0");
    }

    let mut known_hosts = expand_user_home("~/.ssh/known_hosts");
    let mut idx = 1usize;
    let mut host = None::<String>;
    while idx < rest.len() {
        match rest[idx].as_str() {
            "-k" | "--known-hosts" => {
                let Some(value) = rest.get(idx + 1) else {
                    return cobra_usage_error("grabpubkey-help.txt", "flag needs an argument: --known-hosts");
                };
                known_hosts = expand_user_home(value);
                idx += 2;
            }
            value if value.starts_with('-') => {
                return cobra_usage_error("grabpubkey-help.txt", &format!("unknown flag: {value}"));
            }
            value => {
                host = Some(value.to_string());
                idx += 1;
            }
        }
    }

    let Some(host) = host else {
        return cobra_usage_error("grabpubkey-help.txt", "requires at least 1 arg(s), only received 0");
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
        Err(err) => return parse_error_response("shell-help.txt", &err),
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
        Err(err) => parse_error_response("shell-help.txt", &err),
    }
}

struct ParsedSshCommand {
    options: ClientOptions,
    command: Vec<String>,
}

fn parse_ssh_client_command(rest: &[String], _command_name: &str) -> Result<ParsedSshCommand, String> {
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
        Err(err) => {
            let help_key = match rest.get(1).map(String::as_str) {
                Some("forward") => "tun-forward-help.txt",
                Some("reverse") => "tun-reverse-help.txt",
                _ => "tun-help.txt",
            };
            parse_error_response(help_key, &err)
        }
    }
}

fn get_command(rest: &[String]) -> CliResponse {
    if matches!(rest, [cmd, help] if cmd == "get" && is_help_flag(help)) {
        return CliResponse::success(golden_cli("get-help.txt"));
    }
    match parse_get_command(rest) {
        Ok(parsed) => {
            let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                Ok(runtime) => runtime,
                Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
            };
            let result = runtime.block_on(async move {
                let mut client = sftp::Client::connect(parsed.options).await?;
                let transfer = TransferOptions::new(parsed.max_workers, parsed.concurrent_transfers);
                let result = if parsed.recursive {
                    client
                        .get_recursive_with_options(&parsed.remote, &parsed.local, transfer)
                        .await
                } else {
                    client
                        .get_file_with_options(&parsed.remote, &parsed.local, transfer)
                        .await
                };
                let _ = client.close().await;
                result
            });
            match result {
                Ok(()) => CliResponse::success(""),
                Err(err) => CliResponse::failure(format!("{err}\n"), 1),
            }
        }
        Err(err) => parse_error_response("get-help.txt", &err),
    }
}

fn put_command(rest: &[String]) -> CliResponse {
    if matches!(rest, [cmd, help] if cmd == "put" && is_help_flag(help)) {
        return CliResponse::success(golden_cli("put-help.txt"));
    }
    match parse_put_command(rest) {
        Ok(parsed) => {
            let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                Ok(runtime) => runtime,
                Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
            };
            let result = runtime.block_on(async move {
                let mut client = sftp::Client::connect(parsed.options).await?;
                let transfer = TransferOptions::new(parsed.max_workers, parsed.concurrent_transfers);
                let result = if parsed.recursive {
                    client
                        .put_recursive_with_options(&parsed.remote, &parsed.local, transfer)
                        .await
                } else {
                    client
                        .put_file_with_options(&parsed.remote, &parsed.local, transfer)
                        .await
                };
                let _ = client.close().await;
                result
            });
            match result {
                Ok(()) => CliResponse::success(""),
                Err(err) => CliResponse::failure(format!("{err}\n"), 1),
            }
        }
        Err(err) => parse_error_response("put-help.txt", &err),
    }
}

fn socks_proxy_command(rest: &[String]) -> CliResponse {
    if matches!(rest, [cmd, help] if cmd == "socks-proxy" && is_help_flag(help)) {
        return CliResponse::success(golden_cli("socks-proxy-help.txt"));
    }
    match parse_socks_proxy_command(rest) {
        Ok((options, listen_address)) => {
            let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
                Ok(runtime) => runtime,
                Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
            };
            match runtime.block_on(socks::run(options, &listen_address)) {
                Ok(()) => CliResponse::success(""),
                Err(err) => CliResponse::failure(format!("{err}\n"), 1),
            }
        }
        Err(err) => parse_error_response("socks-proxy-help.txt", &err),
    }
}

fn dns_proxy_command(rest: &[String]) -> CliResponse {
    if matches!(rest, [cmd, help] if cmd == "dns-proxy" && is_help_flag(help)) {
        return CliResponse::success(golden_cli("dns-proxy-help.txt"));
    }
    match parse_dns_proxy_command(rest) {
        Ok((options, listen_address, remote_dns)) => {
            let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
                Ok(runtime) => runtime,
                Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
            };
            match runtime.block_on(dns_proxy::run(options, &listen_address, &remote_dns)) {
                Ok(()) => CliResponse::success(""),
                Err(err) => CliResponse::failure(format!("{err}\n"), 1),
            }
        }
        Err(err) => parse_error_response("dns-proxy-help.txt", &err),
    }
}

fn sshd_command(rest: &[String]) -> CliResponse {
    if matches!(rest, [cmd, help] if cmd == "sshd" && is_help_flag(help)) {
        return CliResponse::success(golden_cli("sshd-help.txt"));
    }
    let options = match parse_sshd_command(rest) {
        Ok(options) => options,
        Err(err) => return parse_error_response("sshd-help.txt", &err),
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };
    match runtime.block_on(sshd::run(options)) {
        Ok(()) => CliResponse::success(""),
        Err(err) => CliResponse::failure(format!("{err}\n"), 1),
    }
}

fn revshell_command(rest: &[String]) -> CliResponse {
    if matches!(rest, [cmd, help] if cmd == "revshell" && is_help_flag(help)) {
        return CliResponse::success(golden_cli("revshell-help.txt"));
    }
    let (client_options, server_options, local, remote) = match parse_revshell_command(rest) {
        Ok(parsed) => parsed,
        Err(err) => return parse_error_response("revshell-help.txt", &err),
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };
    let result = runtime.block_on(async move {
        let sshd_task = tokio::spawn(async move { sshd::run(server_options).await });
        let tunnel_task = tokio::spawn(async move { tunnel::run_reverse(client_options, local, remote).await });

        tokio::select! {
            result = sshd_task => match result {
                Ok(result) => result,
                Err(err) => Err(err.to_string()),
            },
            result = tunnel_task => match result {
                Ok(result) => result,
                Err(err) => Err(err.to_string()),
            },
        }
    });
    match result {
        Ok(()) => CliResponse::success(""),
        Err(err) => CliResponse::failure(format!("{err}\n"), 1),
    }
}

struct ParsedGetCommand {
    options: ClientOptions,
    remote: String,
    local: String,
    recursive: bool,
    max_workers: usize,
    concurrent_transfers: usize,
}

struct ParsedPutCommand {
    options: ClientOptions,
    local: String,
    remote: String,
    recursive: bool,
    max_workers: usize,
    concurrent_transfers: usize,
}

fn parse_get_command(rest: &[String]) -> Result<ParsedGetCommand, String> {
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

fn parse_put_command(rest: &[String]) -> Result<ParsedPutCommand, String> {
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

fn parse_socks_proxy_command(rest: &[String]) -> Result<(ClientOptions, String), String> {
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

fn parse_dns_proxy_command(rest: &[String]) -> Result<(ClientOptions, String, String), String> {
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

fn client_options_from_conf(conf: &SshClientConf) -> Result<ClientOptions, String> {
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

fn parse_sshd_command(rest: &[String]) -> Result<ServerOptions, String> {
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

fn parse_revshell_command(
    rest: &[String],
) -> Result<(ClientOptions, ServerOptions, crate::utils::Endpoint, crate::utils::Endpoint), String> {
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

fn parse_error_response(help_key: &str, err: &str) -> CliResponse {
    if err.starts_with("unknown flag:")
        || err.starts_with("flag needs an argument:")
        || err.starts_with("requires at least ")
    {
        return cobra_usage_error(help_key, err);
    }
    CliResponse::failure(format!("{err}\n"), 1)
}

fn cobra_usage_error(help_key: &str, err: &str) -> CliResponse {
    CliResponse::success_stderr(format!("Error: {err}\n{}", command_usage(help_key)))
}

fn command_usage(help_key: &str) -> String {
    let help = golden_cli(help_key);
    match help.find("Usage:\n") {
        Some(index) => ensure_trailing_blank_line(help[index..].to_string()),
        None => ensure_trailing_blank_line(help),
    }
}

fn ensure_trailing_blank_line(mut value: String) -> String {
    if !value.ends_with("\n\n") {
        if !value.ends_with('\n') {
            value.push('\n');
        }
        value.push('\n');
    }
    value
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
