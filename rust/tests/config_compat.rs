use std::fs;
use std::path::PathBuf;

use rospo::config::{load_config_file, Config};

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rust dir has parent")
        .to_path_buf()
}

#[test]
fn parses_go_sshc_fixture() {
    let path = repo_root().join("pkg/conf/testdata/sshc.yaml");
    let content = fs::read_to_string(path).expect("read fixture");
    let config: Config = serde_yaml::from_str(&content).expect("parse yaml");
    assert!(config.ssh_client.is_some());
    assert!(config.sshd.is_none());
    assert_eq!(config.ssh_client.as_ref().expect("ssh client").insecure, false);
}

#[test]
fn parses_go_sshc_insecure_fixture() {
    let path = repo_root().join("pkg/conf/testdata/sshc_insecure.yaml");
    let content = fs::read_to_string(path).expect("read fixture");
    let config: Config = serde_yaml::from_str(&content).expect("parse yaml");
    assert!(config.ssh_client.as_ref().expect("ssh client").insecure);
}

#[test]
fn defaults_missing_bools_to_false() {
    let path = repo_root().join("pkg/conf/testdata/sshc_secure_default.yaml");
    let content = fs::read_to_string(path).expect("read fixture");
    let config: Config = serde_yaml::from_str(&content).expect("parse yaml");
    assert!(!config.ssh_client.as_ref().expect("ssh client").insecure);
}

#[test]
fn sshd_disable_shell_defaults_to_false() {
    let config = load_config_file(&repo_root().join("pkg/conf/testdata/sshd.yaml")).expect("load sshd config");
    assert!(!config.sshd.as_ref().expect("sshd config").disable_shell);
}

#[test]
fn empty_sshclient_is_none() {
    let config = load_config_file(&repo_root().join("pkg/conf/testdata/sshd.yaml")).expect("load sshd config");
    assert!(config.ssh_client.is_none());
}

#[test]
fn fails_on_nonexistent_and_unparsable_config_files() {
    assert!(load_config_file(&repo_root().join("pkg/conf/testdata/not_existent.yaml")).is_err());
    assert!(load_config_file(&repo_root().join("pkg/conf/testdata/unparsable.yaml")).is_err());
}
