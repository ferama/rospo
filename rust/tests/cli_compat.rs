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
fn root_help_matches_go_fixture() {
    let response = execute(["rospo", "--help"]);
    assert_eq!(response.exit_code, 0);
    assert_eq!(response.stdout, golden("root-help.txt"));
    assert!(response.stderr.is_empty());
}

#[test]
fn root_noargs_matches_go_fixture() {
    let response = execute(["rospo"]);
    assert_eq!(response.exit_code, 0);
    assert_eq!(response.stdout, golden("root-noargs.txt"));
    assert!(response.stderr.is_empty());
}

#[test]
fn tun_forward_help_matches_go_fixture() {
    let response = execute(["rospo", "tun", "forward", "--help"]);
    assert_eq!(response.exit_code, 0);
    assert_eq!(response.stdout, golden("tun-forward-help.txt"));
}

#[test]
fn template_output_matches_go_fixture() {
    let response = execute(["rospo", "template"]);
    assert_eq!(response.exit_code, 0);
    assert_eq!(response.stdout, golden("template-output.txt"));
}

#[test]
fn all_captured_help_outputs_match_go_fixtures() {
    let cases = [
        (vec!["rospo", "dns-proxy", "--help"], "dns-proxy-help.txt"),
        (vec!["rospo", "get", "--help"], "get-help.txt"),
        (vec!["rospo", "grabpubkey", "--help"], "grabpubkey-help.txt"),
        (vec!["rospo", "keygen", "--help"], "keygen-help.txt"),
        (vec!["rospo", "put", "--help"], "put-help.txt"),
        (vec!["rospo", "revshell", "--help"], "revshell-help.txt"),
        (vec!["rospo", "run", "--help"], "run-help.txt"),
        (vec!["rospo", "shell", "--help"], "shell-help.txt"),
        (vec!["rospo", "socks-proxy", "--help"], "socks-proxy-help.txt"),
        (vec!["rospo", "sshd", "--help"], "sshd-help.txt"),
        (vec!["rospo", "template", "--help"], "template-help.txt"),
        (vec!["rospo", "tun", "--help"], "tun-help.txt"),
        (vec!["rospo", "tun", "forward", "--help"], "tun-forward-help.txt"),
        (vec!["rospo", "tun", "reverse", "--help"], "tun-reverse-help.txt"),
    ];

    for (args, fixture) in cases {
        let response = execute(args);
        assert_eq!(response.exit_code, 0, "fixture {fixture}");
        assert_eq!(response.stdout, golden(fixture), "fixture {fixture}");
        assert!(response.stderr.is_empty(), "fixture {fixture}");
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
