use std::path::PathBuf;

use internal_russh_forked_ssh_key::{public::EcdsaPublicKey, PublicKey};
use p521::ecdsa::VerifyingKey;
use p521::elliptic_curve::rand_core::OsRng;
use p521::elliptic_curve::sec1::ToEncodedPoint;
use sec1::LineEnding;

use crate::cli::CliResponse;
use crate::utils::{expand_user_home, write_file_0600};

pub(crate) fn keygen_command(rest: &[String]) -> CliResponse {
    if matches!(rest, [cmd, help] if cmd == "keygen" && super::super::help::is_help_flag(help)) {
        return CliResponse::success(super::super::golden_cli("keygen-help.txt"));
    }

    let mut store = false;
    let mut path = ".".to_string();
    let mut name = "identity".to_string();

    let mut idx = 1usize;
    while idx < rest.len() {
        match rest[idx].as_str() {
            "-s" | "--store" => {
                store = true;
                idx += 1;
            }
            "-p" | "--path" => {
                let Some(value) = rest.get(idx + 1) else {
                    return super::super::help::cobra_usage_error("keygen-help.txt", "flag needs an argument: --path");
                };
                path = value.clone();
                idx += 2;
            }
            "-n" | "--name" => {
                let Some(value) = rest.get(idx + 1) else {
                    return super::super::help::cobra_usage_error("keygen-help.txt", "flag needs an argument: --name");
                };
                name = value.clone();
                idx += 2;
            }
            other => {
                return super::super::help::cobra_usage_error("keygen-help.txt", &format!("unknown flag: {other}"));
            }
        }
    }

    let secret = p521::SecretKey::random(&mut OsRng);
    let private_pem = match secret.to_sec1_pem(LineEnding::LF) {
        Ok(pem) => pem.to_string(),
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };
    let encoded_point = secret.public_key().to_encoded_point(false);
    let verifying_key = match VerifyingKey::from_encoded_point(&encoded_point) {
        Ok(key) => key,
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };
    let public_key = PublicKey::from(EcdsaPublicKey::from(&verifying_key));
    let public_key = match public_key.to_openssh() {
        Ok(encoded) => format!("{encoded}\n"),
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };

    if store {
        let dir = PathBuf::from(expand_user_home(&path));
        let _ = write_file_0600(&dir.join(&name), private_pem.as_bytes());
        let _ = write_file_0600(&dir.join(format!("{name}.pub")), public_key.as_bytes());
        CliResponse::success("")
    } else {
        CliResponse::success(format!("{private_pem}{public_key}"))
    }
}
