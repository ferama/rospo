use crate::cli::app::{DnsProxyArgs, SocksProxyArgs};
use crate::cli::parse::client_options_from_cli;
use crate::cli::CliResponse;
use crate::dns_proxy;
use crate::socks;

pub(crate) fn socks_proxy_command(args: SocksProxyArgs) -> CliResponse {
    let options = match client_options_from_cli(&args.server, &args.ssh) {
        Ok(options) => options,
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };
    match runtime.block_on(socks::run(options, &args.listen_address)) {
        Ok(()) => CliResponse::success(""),
        Err(err) => CliResponse::failure(format!("{err}\n"), 1),
    }
}

pub(crate) fn dns_proxy_command(args: DnsProxyArgs) -> CliResponse {
    let options = match client_options_from_cli(&args.server, &args.ssh) {
        Ok(options) => options,
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };
    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };
    match runtime.block_on(dns_proxy::run(
        options,
        &args.listen_address,
        &args.remote_dns_server,
    )) {
        Ok(()) => CliResponse::success(""),
        Err(err) => CliResponse::failure(format!("{err}\n"), 1),
    }
}
