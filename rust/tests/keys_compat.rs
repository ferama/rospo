use std::path::PathBuf;

use rospo::ssh::load_secret_key;
use rospo::utils::write_file_0600;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rust dir has parent")
        .to_path_buf()
}

#[test]
fn load_identity_file_succeeds_for_existing_key() {
    let key = load_secret_key(&repo_root().join("testdata/client"), None).expect("load private key");
    assert_eq!(
        key.public_key()
            .to_openssh()
            .expect("serialize public key")
            .split_whitespace()
            .next()
            .expect("public key algorithm"),
        "ssh-rsa"
    );
}

#[test]
fn load_identity_file_fails_for_missing_key() {
    assert!(load_secret_key(&repo_root().join("testdata/does-not-exist"), None).is_err());
}

#[test]
fn write_file_0600_creates_files() {
    let path = std::env::temp_dir().join(format!("rospo-key-write-{}", std::process::id()));
    let _ = std::fs::remove_file(&path);
    write_file_0600(&path, b"test").expect("write temp file");
    assert_eq!(std::fs::read(&path).expect("read back temp file"), b"test");
    let _ = std::fs::remove_file(path);
}
