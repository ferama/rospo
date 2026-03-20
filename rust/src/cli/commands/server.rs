use crate::cli::parse::{parse_revshell_command, parse_sshd_command};
use crate::cli::CliResponse;
use crate::sshd;
use crate::tunnel;

pub(crate) fn sshd_command(rest: &[String]) -> CliResponse {
    if matches!(rest, [cmd, help] if cmd == "sshd" && super::super::help::is_help_flag(help)) {
        return CliResponse::success(super::super::golden_cli("sshd-help.txt"));
    }
    let options = match parse_sshd_command(rest) {
        Ok(options) => options,
        Err(err) => return super::super::help::parse_error_response("sshd-help.txt", &err),
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

pub(crate) fn revshell_command(rest: &[String]) -> CliResponse {
    if matches!(rest, [cmd, help] if cmd == "revshell" && super::super::help::is_help_flag(help)) {
        return CliResponse::success(super::super::golden_cli("revshell-help.txt"));
    }
    let (client_options, server_options, local, remote) = match parse_revshell_command(rest) {
        Ok(parsed) => parsed,
        Err(err) => return super::super::help::parse_error_response("revshell-help.txt", &err),
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
