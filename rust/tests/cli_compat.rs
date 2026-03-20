use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rospo::cli::execute;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rust dir has parent")
        .to_path_buf()
}

fn golden(name: &str) -> String {
    std::fs::read_to_string(repo_root().join("compat/golden/cli").join(name)).expect("read cli fixture")
}

#[test]
fn root_help_lists_commands() {
    let response = execute(["rospo", "--help"]);
    assert_eq!(response.exit_code, 0);
    assert!(response.stdout.contains("Usage: rospo"));
    assert!(response.stdout.contains("run"));
    assert!(response.stdout.contains("template"));
    assert!(response.stdout.contains("socks-proxy"));
    assert!(response.stdout.contains("dns-proxy"));
    assert!(response.stderr.is_empty());
}

#[test]
fn root_quiet_flag_is_accepted_before_subcommands() {
    let response = execute(["rospo", "-q", "template"]);
    assert_eq!(response.exit_code, 0);
    assert_eq!(response.stdout, golden("template-output.txt"));
    assert!(response.stderr.is_empty());
}

#[test]
fn root_noargs_prints_clap_help() {
    let response = execute(["rospo"]);
    assert_eq!(response.exit_code, 0);
    assert!(response.stdout.contains("Usage: rospo"));
    assert!(response.stdout.contains("Commands:"));
    assert!(response.stderr.is_empty());
}

#[test]
fn tun_forward_help_is_generated_by_clap() {
    let response = execute(["rospo", "tun", "forward", "--help"]);
    assert_eq!(response.exit_code, 0);
    assert!(response.stdout.contains("Usage: rospo tun forward"));
    assert!(response.stdout.contains("--local"));
    assert!(response.stdout.contains("--remote"));
}

#[test]
fn template_output_matches_go_fixture() {
    let response = execute(["rospo", "template"]);
    assert_eq!(response.exit_code, 0);
    assert_eq!(response.stdout, golden("template-output.txt"));
}

#[test]
fn command_help_outputs_are_available() {
    let cases = [
        vec!["rospo", "dns-proxy", "--help"],
        vec!["rospo", "get", "--help"],
        vec!["rospo", "grabpubkey", "--help"],
        vec!["rospo", "keygen", "--help"],
        vec!["rospo", "put", "--help"],
        vec!["rospo", "revshell", "--help"],
        vec!["rospo", "run", "--help"],
        vec!["rospo", "shell", "--help"],
        vec!["rospo", "socks-proxy", "--help"],
        vec!["rospo", "sshd", "--help"],
        vec!["rospo", "template", "--help"],
        vec!["rospo", "tun", "--help"],
        vec!["rospo", "tun", "forward", "--help"],
        vec!["rospo", "tun", "reverse", "--help"],
        vec!["rospo", "help", "template"],
    ];

    for args in cases {
        let response = execute(args.clone());
        assert_eq!(response.exit_code, 0, "args: {args:?}");
        assert!(response.stdout.contains("Usage:"), "args: {args:?}");
        assert!(response.stderr.is_empty(), "args: {args:?}");
    }
}

#[test]
fn keygen_outputs_p521_private_and_public_keys() {
    let response = execute(["rospo", "keygen"]);
    assert_eq!(response.exit_code, 0);
    assert!(response.stderr.is_empty());
    assert!(response.stdout.starts_with("-----BEGIN EC PRIVATE KEY-----\n"));
    assert!(response.stdout.contains("\necdsa-sha2-nistp521 "));
}

#[test]
fn keygen_store_writes_expected_files() {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!("rospo-keygen-{unique}"));
    std::fs::create_dir_all(&dir).expect("create temp dir");

    let response = execute([
        "rospo",
        "keygen",
        "--store",
        "--path",
        dir.to_str().expect("temp dir utf8"),
        "--name",
        "identity",
    ]);

    assert_eq!(response.exit_code, 0);
    assert!(response.stdout.is_empty());
    assert!(response.stderr.is_empty());

    let private = std::fs::read_to_string(dir.join("identity")).expect("read private key");
    let public = std::fs::read_to_string(dir.join("identity.pub")).expect("read public key");
    assert!(private.starts_with("-----BEGIN EC PRIVATE KEY-----\n"));
    assert!(public.starts_with("ecdsa-sha2-nistp521 "));

    let _ = std::fs::remove_file(dir.join("identity"));
    let _ = std::fs::remove_file(dir.join("identity.pub"));
    let _ = std::fs::remove_dir(dir);
}

#[test]
fn malformed_invocations_fail_with_clap_errors() {
    let cases = [
        (vec!["rospo", "run"], "Usage: rospo run <CONFIG>"),
        (vec!["rospo", "keygen", "--bad"], "unexpected argument '--bad'"),
        (vec!["rospo", "grabpubkey"], "Usage: rospo grabpubkey"),
        (vec!["rospo", "shell"], "Usage: rospo shell"),
        (vec!["rospo", "get"], "Usage: rospo get"),
        (vec!["rospo", "put"], "Usage: rospo put"),
        (vec!["rospo", "socks-proxy"], "Usage: rospo socks-proxy"),
        (vec!["rospo", "dns-proxy"], "Usage: rospo dns-proxy"),
        (vec!["rospo", "tun"], "Usage: rospo tun"),
        (vec!["rospo", "tun", "forward"], "Usage: rospo tun forward"),
        (vec!["rospo", "tun", "reverse"], "Usage: rospo tun reverse"),
        (vec!["rospo", "sshd", "--bad"], "unexpected argument '--bad'"),
        (vec!["rospo", "revshell"], "Usage: rospo revshell"),
        (vec!["rospo", "bogus"], "unrecognized subcommand 'bogus'"),
    ];

    for (args, expected) in cases {
        let rust = execute(args.clone());
        assert_ne!(rust.exit_code, 0, "args: {args:?}");
        assert!(rust.stderr.contains(expected), "args: {args:?}\nstderr: {}", rust.stderr);
    }
}
