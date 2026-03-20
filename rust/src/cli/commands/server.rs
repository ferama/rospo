use crate::cli::app::{RevshellArgs, SshdArgs};
use crate::cli::parse::{client_options_from_cli, server_options_from_cli};
use crate::cli::CliResponse;
use crate::sshd;
use crate::tunnel;
use crate::utils::new_endpoint;

pub(crate) fn sshd_command(args: SshdArgs) -> CliResponse {
    let options = server_options_from_cli(&args.sshd);
    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };
    match runtime.block_on(sshd::run(options)) {
        Ok(()) => CliResponse::success(""),
        Err(err) => CliResponse::failure(format!("{err}\n"), 1),
    }
}

pub(crate) fn revshell_command(args: RevshellArgs) -> CliResponse {
    let client_options = match client_options_from_cli(&args.server, &args.ssh) {
        Ok(options) => options,
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };
    let server_options = server_options_from_cli(&args.sshd);
    let local = match new_endpoint(&args.sshd.sshd_listen_address) {
        Ok(endpoint) => endpoint,
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };
    let remote = match new_endpoint(&args.remote) {
        Ok(endpoint) => endpoint,
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };

    let runtime = match tokio::runtime::Builder::new_multi_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };
    let result = runtime.block_on(async move {
        // revshell follows the Go composition model: start a local embedded sshd, then keep a
        // reverse tunnel alive so the remote endpoint can dial back into that local server.
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
