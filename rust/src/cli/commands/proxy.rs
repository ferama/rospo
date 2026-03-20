use crate::cli::parse::{parse_dns_proxy_command, parse_socks_proxy_command};
use crate::cli::CliResponse;
use crate::dns_proxy;
use crate::socks;

pub(crate) fn socks_proxy_command(rest: &[String]) -> CliResponse {
    if matches!(rest, [cmd, help] if cmd == "socks-proxy" && super::super::help::is_help_flag(help)) {
        return CliResponse::success(super::super::golden_cli("socks-proxy-help.txt"));
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
        Err(err) => super::super::help::parse_error_response("socks-proxy-help.txt", &err),
    }
}

pub(crate) fn dns_proxy_command(rest: &[String]) -> CliResponse {
    if matches!(rest, [cmd, help] if cmd == "dns-proxy" && super::super::help::is_help_flag(help)) {
        return CliResponse::success(super::super::golden_cli("dns-proxy-help.txt"));
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
        Err(err) => super::super::help::parse_error_response("dns-proxy-help.txt", &err),
    }
}
