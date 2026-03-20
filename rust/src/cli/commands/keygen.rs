use std::path::PathBuf;

use internal_russh_forked_ssh_key::{public::EcdsaPublicKey, PublicKey};
use p521::ecdsa::VerifyingKey;
use p521::elliptic_curve::rand_core::OsRng;
use p521::elliptic_curve::sec1::ToEncodedPoint;
use sec1::LineEnding;

use crate::cli::app::KeygenArgs;
use crate::cli::CliResponse;
use crate::utils::{expand_user_home, write_file_0600};

pub(crate) fn keygen_command(args: KeygenArgs) -> CliResponse {
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

    if args.store {
        let dir = PathBuf::from(expand_user_home(&args.path));
        let _ = write_file_0600(&dir.join(&args.name), private_pem.as_bytes());
        let _ = write_file_0600(&dir.join(format!("{}.pub", args.name)), public_key.as_bytes());
        CliResponse::success("")
    } else {
        CliResponse::success(format!("{private_pem}{public_key}"))
    }
}
