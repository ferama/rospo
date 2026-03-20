use std::fs;
use std::path::PathBuf;

use super::CliResponse;

pub(crate) fn matches_help(rest: &[String]) -> bool {
    matches!(rest, [flag] if is_help_flag(flag))
}

pub(crate) fn is_help_flag(flag: &str) -> bool {
    flag == "-h" || flag == "--help"
}

pub(crate) fn command_help_key(path: &[&str]) -> Option<&'static str> {
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

pub(crate) fn parse_error_response(help_key: &str, err: &str) -> CliResponse {
    if err.starts_with("unknown flag:")
        || err.starts_with("flag needs an argument:")
        || err.starts_with("requires at least ")
    {
        return cobra_usage_error(help_key, err);
    }
    CliResponse::failure(format!("{err}\n"), 1)
}

pub(crate) fn cobra_usage_error(help_key: &str, err: &str) -> CliResponse {
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

pub(crate) fn golden_cli(name: &str) -> String {
    fs::read_to_string(repo_root().join("compat/golden/cli").join(name)).expect("read cli fixture")
}

pub(crate) fn template_output() -> String {
    fs::read_to_string(repo_root().join("cmd/configs/config_template.yaml"))
        .expect("read config template")
        + "\n"
}
