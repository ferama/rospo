use std::ffi::OsString;

use crate::logging;

mod commands;
mod help;
mod parse;
mod response;

pub(crate) use help::{golden_cli, matches_help, template_output};
use help::command_help_key;
pub use response::CliResponse;

pub const VERSION: &str = "development";

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

    if matches!(rest, [cmd, help] if cmd == "template" && help::is_help_flag(help)) {
        return CliResponse::success(golden_cli("template-help.txt"));
    }
    if matches!(rest, [cmd] if cmd == "template") {
        return CliResponse::success(template_output());
    }

    let help_key = match rest {
        [cmd, help] if help::is_help_flag(help) => command_help_key(&[cmd.as_str()]),
        [cmd1, cmd2, help] if help::is_help_flag(help) => command_help_key(&[cmd1.as_str(), cmd2.as_str()]),
        [cmd] if cmd == "help" => Some("root-help.txt"),
        [cmd, topic] if cmd == "help" => command_help_key(&[topic.as_str()]),
        [cmd, topic, subtopic] if cmd == "help" => command_help_key(&[topic.as_str(), subtopic.as_str()]),
        _ => None,
    };
    if let Some(help_key) = help_key {
        return CliResponse::success(golden_cli(help_key));
    }

    match rest.first().map(String::as_str) {
        Some("run") => commands::config::run_config_command(rest),
        Some("keygen") => commands::keygen::keygen_command(rest),
        Some("grabpubkey") => commands::shell::grabpubkey_command(rest),
        Some("shell") => commands::shell::shell_command(rest),
        Some("get") => commands::transfer::get_command(rest),
        Some("put") => commands::transfer::put_command(rest),
        Some("socks-proxy") => commands::proxy::socks_proxy_command(rest),
        Some("dns-proxy") => commands::proxy::dns_proxy_command(rest),
        Some("tun") => commands::transfer::tun_command(rest),
        Some("sshd") => commands::server::sshd_command(rest),
        Some("revshell") => commands::server::revshell_command(rest),
        _ => CliResponse {
            stdout: "invalid subcommand\n".to_string(),
            stderr: String::new(),
            exit_code: 1,
        },
    }
}
