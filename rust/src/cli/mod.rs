use std::ffi::OsString;

use clap::{CommandFactory, Parser};

use crate::logging;

pub mod app;
mod commands;
mod help;
mod parse;
mod response;

pub use app::Cli;
use app::{Command, HelpArgs};
pub(crate) use help::template_output;
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
    let cli = match Cli::try_parse_from(args) {
        Ok(cli) => cli,
        Err(err) => {
            let message = err.to_string();
            let code = err.exit_code();
            if err.use_stderr() {
                return CliResponse::failure(message, code);
            }
            return CliResponse::success(message);
        }
    };
    dispatch(cli)
}

pub fn run<I, T>(args: I) -> i32
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let cli = Cli::parse_from(args);
    run_cli(cli)
}

pub fn run_cli(cli: Cli) -> i32 {
    let response = dispatch(cli);
    if !response.stdout.is_empty() {
        print!("{}", response.stdout);
    }
    if !response.stderr.is_empty() {
        eprint!("{}", response.stderr);
    }
    response.exit_code
}

fn dispatch(cli: Cli) -> CliResponse {
    logging::init_logging(cli.quiet);

    match cli.command {
        None => CliResponse::success(render_help(&[])),
        Some(Command::Help(args)) => CliResponse::success(render_help_path(args)),
        Some(Command::Template) => CliResponse::success(template_output()),
        Some(Command::Run(args)) => commands::config::run_config_command(&args.config),
        Some(Command::Keygen(args)) => commands::keygen::keygen_command(args),
        Some(Command::Grabpubkey(args)) => commands::shell::grabpubkey_command(args),
        Some(Command::Shell(args)) => commands::shell::shell_command(args),
        Some(Command::Get(args)) => commands::transfer::get_command(args),
        Some(Command::Put(args)) => commands::transfer::put_command(args),
        Some(Command::SocksProxy(args)) => commands::proxy::socks_proxy_command(args),
        Some(Command::DnsProxy(args)) => commands::proxy::dns_proxy_command(args),
        Some(Command::Tun(args)) => commands::transfer::tun_command(args),
        Some(Command::Sshd(args)) => commands::server::sshd_command(args),
        Some(Command::Revshell(args)) => commands::server::revshell_command(args),
    }
}

fn render_help_path(args: HelpArgs) -> String {
    match (args.command.as_deref(), args.subcommand.as_deref()) {
        (None, _) => render_help(&[]),
        (Some(command), None) => render_help(&[command]),
        (Some(command), Some(subcommand)) => render_help(&[command, subcommand]),
    }
}

fn render_help(path: &[&str]) -> String {
    let mut command = Cli::command();
    let mut current = &mut command;
    for segment in path {
        let Some(next) = current.find_subcommand_mut(segment) else {
            return format!("unknown help topic: {}\n", path.join(" "));
        };
        current = next;
    }

    let mut buf = Vec::new();
    current.write_long_help(&mut buf).expect("render clap help");
    if !buf.ends_with(b"\n") {
        buf.push(b'\n');
    }
    String::from_utf8(buf).expect("utf8 clap help")
}
