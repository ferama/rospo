use std::path::Path;

use crate::cli::parse::parse_ssh_client_command;
use crate::cli::CliResponse;
use crate::ssh::{fetch_server_public_key, Session};
use crate::utils::{add_host_key_to_known_hosts, expand_user_home, parse_ssh_url};

pub(crate) fn grabpubkey_command(rest: &[String]) -> CliResponse {
    if matches!(rest, [cmd, help] if cmd == "grabpubkey" && super::super::help::is_help_flag(help)) {
        return CliResponse::success(super::super::golden_cli("grabpubkey-help.txt"));
    }
    if rest.len() < 2 {
        return super::super::help::cobra_usage_error("grabpubkey-help.txt", "requires at least 1 arg(s), only received 0");
    }

    let mut known_hosts = expand_user_home("~/.ssh/known_hosts");
    let mut idx = 1usize;
    let mut host = None::<String>;
    while idx < rest.len() {
        match rest[idx].as_str() {
            "-k" | "--known-hosts" => {
                let Some(value) = rest.get(idx + 1) else {
                    return super::super::help::cobra_usage_error("grabpubkey-help.txt", "flag needs an argument: --known-hosts");
                };
                known_hosts = expand_user_home(value);
                idx += 2;
            }
            value if value.starts_with('-') => {
                return super::super::help::cobra_usage_error("grabpubkey-help.txt", &format!("unknown flag: {value}"));
            }
            value => {
                host = Some(value.to_string());
                idx += 1;
            }
        }
    }

    let Some(host) = host else {
        return super::super::help::cobra_usage_error("grabpubkey-help.txt", "requires at least 1 arg(s), only received 0");
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

pub(crate) fn shell_command(rest: &[String]) -> CliResponse {
    if matches!(rest, [cmd, help] if cmd == "shell" && super::super::help::is_help_flag(help)) {
        return CliResponse::success(super::super::golden_cli("shell-help.txt"));
    }

    let parsed = match parse_ssh_client_command(rest, "shell") {
        Ok(parsed) => parsed,
        Err(err) => return super::super::help::parse_error_response("shell-help.txt", &err),
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
        Err(err) => super::super::help::parse_error_response("shell-help.txt", &err),
    }
}
