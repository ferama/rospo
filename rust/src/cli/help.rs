use std::fs;
use std::path::PathBuf;

pub(crate) fn template_output() -> String {
    fs::read_to_string(repo_root().join("cmd/configs/config_template.yaml"))
        .expect("read config template")
        + "\n"
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rust dir has parent")
        .to_path_buf()
}
