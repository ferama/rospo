use std::fs;
use std::path::PathBuf;

use internal_russh_forked_ssh_key::PublicKey;
use rospo::utils::{add_host_key_to_known_hosts, new_endpoint, parse_ssh_config_file, parse_ssh_url, serialize_public_key};
use serde_json::Value;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rust dir has parent")
        .to_path_buf()
}

fn read_json(path: &str) -> Value {
    let content = fs::read_to_string(repo_root().join(path)).expect("read json fixture");
    serde_json::from_str(&content).expect("parse json fixture")
}

#[test]
fn ssh_url_matches_ipv4_fixture() {
    let expected = read_json("compat/golden/runtime/ssh_url_ipv4.json");
    let parsed = parse_ssh_url("user@192.168.0.1:22").expect("parse ssh url");
    assert_eq!(parsed.username, expected["Username"].as_str().expect("username"));
    assert_eq!(parsed.host, expected["Host"].as_str().expect("host"));
    assert_eq!(parsed.port, expected["Port"].as_u64().expect("port") as u16);
}

#[test]
fn ssh_url_matches_empty_host_fixture() {
    let expected = read_json("compat/golden/runtime/ssh_url_empty_host.json");
    let parsed = parse_ssh_url(":22").expect("parse ssh url");
    assert_eq!(parsed.username, expected["Username"].as_str().expect("username"));
    assert_eq!(parsed.host, expected["Host"].as_str().expect("host"));
    assert_eq!(parsed.port, expected["Port"].as_u64().expect("port") as u16);
}

#[test]
fn ssh_url_matches_ipv6_fixture() {
    let expected = read_json("compat/golden/runtime/ssh_url_ipv6.json");
    let parsed = parse_ssh_url("user@[2001:0db8:85a3:0000:0000:8a2e:0370:7334]:2222").expect("parse ssh url");
    assert_eq!(parsed.username, expected["Username"].as_str().expect("username"));
    assert_eq!(parsed.host, expected["Host"].as_str().expect("host"));
    assert_eq!(parsed.port, expected["Port"].as_u64().expect("port") as u16);
}

#[test]
fn endpoint_string_matches_go_behavior() {
    let endpoint = new_endpoint("localhost:2222").expect("parse endpoint");
    assert_eq!(endpoint.to_string(), "localhost:2222");
}

#[test]
fn ssh_config_parser_matches_fixture() {
    let parsed = parse_ssh_config_file(&repo_root().join("pkg/utils/testdata/ssh_config")).expect("parse ssh config");
    let expected = read_json("compat/golden/runtime/ssh_config.json")
        .as_array()
        .expect("ssh config fixture array")
        .clone();

    assert_eq!(parsed.len(), expected.len());
    for (node, expected) in parsed.iter().zip(expected.iter()) {
        assert_eq!(node.host, expected["Host"].as_str().expect("host"));
        assert_eq!(node.port, expected["Port"].as_u64().expect("port") as u16);
        assert_eq!(node.host_name, expected["HostName"].as_str().expect("hostname"));
        assert_eq!(node.user, expected["User"].as_str().expect("user"));
        assert_eq!(node.identity_file, expected["IdentityFile"].as_str().expect("identity"));
        assert_eq!(
            node.strict_host_key_checking,
            expected["StrictHostKeyChecking"].as_bool().expect("strict host key checking")
        );
        assert_eq!(
            node.user_known_hosts_file,
            expected["UserKnownHostsFile"].as_str().expect("known hosts")
        );
        assert_eq!(node.proxy_jump, expected["ProxyJump"].as_str().expect("proxy jump"));
    }
}

#[test]
fn serialize_public_key_matches_openssh_format() {
    let key = PublicKey::from_openssh("ecdsa-sha2-nistp521 AAAAE2VjZHNhLXNoYTItbmlzdHA1MjEAAAAIbmlzdHA1MjEAAACFBAHBaZ+Ukz6tkl/ihAzM6+s/8roClWv97z0dAILllHK7c2I6oYdGNMEsmQsnazrnMgKWnepSwt8AHgblYly7RziWtgHNxXR9CtCSrw5EwOQ1KDZl1OsOWtuLzjeU3DN0igLiVNCuT8NRWMndGmDVxD5xOHRXrahn11zZOcQ3gg44c/JRAA==").expect("parse public key");
    let serialized = serialize_public_key(&key).expect("serialize public key");
    assert!(serialized.starts_with("ecdsa-sha2-nistp521 "));
}

#[test]
fn add_host_key_uses_go_known_hosts_format() {
    let key = PublicKey::from_openssh("ecdsa-sha2-nistp521 AAAAE2VjZHNhLXNoYTItbmlzdHA1MjEAAAAIbmlzdHA1MjEAAACFBAHBaZ+Ukz6tkl/ihAzM6+s/8roClWv97z0dAILllHK7c2I6oYdGNMEsmQsnazrnMgKWnepSwt8AHgblYly7RziWtgHNxXR9CtCSrw5EwOQ1KDZl1OsOWtuLzjeU3DN0igLiVNCuT8NRWMndGmDVxD5xOHRXrahn11zZOcQ3gg44c/JRAA==").expect("parse public key");
    let path = std::env::temp_dir().join("rospo-known-hosts-test");
    let _ = std::fs::remove_file(&path);

    add_host_key_to_known_hosts("127.0.0.1:22", &key, &path).expect("append known hosts");
    let content = std::fs::read_to_string(&path).expect("read known hosts");
    assert!(content.starts_with("127.0.0.1 ecdsa-sha2-nistp521 "));

    let _ = std::fs::remove_file(path);
}
