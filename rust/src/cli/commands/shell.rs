use std::path::Path;

use crate::cli::app::{GrabPubkeyArgs, ShellArgs};
use crate::cli::parse::client_options_from_cli;
use crate::cli::CliResponse;
use crate::ssh::{fetch_server_public_key, Session};
use crate::utils::{add_host_key_to_known_hosts, expand_user_home, parse_ssh_url};

pub(crate) fn grabpubkey_command(args: GrabPubkeyArgs) -> CliResponse {
    let known_hosts = expand_user_home(&args.known_hosts);
    let parsed = match parse_ssh_url(&args.server) {
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
    let host = format!("{}:{}", parsed.host, parsed.port);
    match add_host_key_to_known_hosts(&host, &key, Path::new(&known_hosts)) {
        Ok(()) => CliResponse::success(""),
        Err(err) => CliResponse::failure(format!("{err}\n"), 1),
    }
}

pub(crate) fn shell_command(args: ShellArgs) -> CliResponse {
    let options = match client_options_from_cli(&args.server, &args.ssh) {
        Ok(options) => options,
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };

    let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };

    let result = runtime.block_on(async move {
        let mut session = Session::connect(options).await?;
        let code = if args.command.is_empty() {
            session.run_shell().await?
        } else {
            session.run_command(&args.command.join(" ")).await?
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
