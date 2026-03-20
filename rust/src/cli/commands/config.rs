use std::fs;
use std::path::Path;

use crate::cli::parse::client_options_from_conf;
use crate::cli::CliResponse;
use crate::config::{load_config, Config};
use crate::dns_proxy;
use crate::socks;
use crate::sshd::{self, ServerOptions};
use crate::tunnel;
use crate::utils::new_endpoint;

pub(crate) fn run_config_command(rest: &[String]) -> CliResponse {
    if matches!(rest, [cmd, help] if cmd == "run" && super::super::help::is_help_flag(help)) {
        return CliResponse::success(super::super::golden_cli("run-help.txt"));
    }
    if rest.len() < 2 {
        return super::super::help::cobra_usage_error("run-help.txt", "requires at least 1 arg(s), only received 0");
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
