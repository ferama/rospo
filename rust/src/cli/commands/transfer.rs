use crate::cli::parse::{parse_get_command, parse_put_command, parse_tun_command};
use crate::cli::CliResponse;
use crate::sftp::{self, TransferOptions};
use crate::tunnel;

use super::transfer_progress::ProgressManager;

pub(crate) fn tun_command(rest: &[String]) -> CliResponse {
    match parse_tun_command(rest) {
        Ok(parsed) => {
            let runtime = match tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build() {
                Ok(runtime) => runtime,
                Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
            };

            let result = runtime.block_on(async move {
                if parsed.forward {
                    tunnel::run_forward(parsed.options, parsed.local, parsed.remote).await
                } else {
                    tunnel::run_reverse(parsed.options, parsed.local, parsed.remote).await
                }
            });

            match result {
                Ok(()) => CliResponse::success(""),
                Err(err) => CliResponse::failure(format!("{err}\n"), 1),
            }
        }
        Err(err) => {
            let help_key = match rest.get(1).map(String::as_str) {
                Some("forward") => "tun-forward-help.txt",
                Some("reverse") => "tun-reverse-help.txt",
                _ => "tun-help.txt",
            };
            super::super::help::parse_error_response(help_key, &err)
        }
    }
}

pub(crate) fn get_command(rest: &[String]) -> CliResponse {
    if matches!(rest, [cmd, help] if cmd == "get" && super::super::help::is_help_flag(help)) {
        return CliResponse::success(super::super::golden_cli("get-help.txt"));
    }
    match parse_get_command(rest) {
        Ok(parsed) => {
            let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                Ok(runtime) => runtime,
                Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
            };
            let result = runtime.block_on(async move {
                let progress = std::sync::Arc::new(ProgressManager::download());
                let mut client = sftp::Client::connect(parsed.options).await?;
                let transfer = TransferOptions::new(parsed.max_workers, parsed.concurrent_transfers);
                let result = if parsed.recursive {
                    client
                        .get_recursive_with_options_and_progress(
                            &parsed.remote,
                            &parsed.local,
                            transfer,
                            Some(progress.clone()),
                        )
                        .await
                } else {
                    client
                        .get_file_with_options_and_progress(
                            &parsed.remote,
                            &parsed.local,
                            transfer,
                            Some(progress.clone()),
                        )
                        .await
                };
                let _ = progress.finish().await;
                let _ = client.close().await;
                result
            });
            match result {
                Ok(()) => CliResponse::success(""),
                Err(err) => CliResponse::failure(format!("{err}\n"), 1),
            }
        }
        Err(err) => super::super::help::parse_error_response("get-help.txt", &err),
    }
}

pub(crate) fn put_command(rest: &[String]) -> CliResponse {
    if matches!(rest, [cmd, help] if cmd == "put" && super::super::help::is_help_flag(help)) {
        return CliResponse::success(super::super::golden_cli("put-help.txt"));
    }
    match parse_put_command(rest) {
        Ok(parsed) => {
            let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
                Ok(runtime) => runtime,
                Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
            };
            let result = runtime.block_on(async move {
                let progress = std::sync::Arc::new(ProgressManager::upload());
                let mut client = sftp::Client::connect(parsed.options).await?;
                let transfer = TransferOptions::new(parsed.max_workers, parsed.concurrent_transfers);
                let result = if parsed.recursive {
                    client
                        .put_recursive_with_options_and_progress(
                            &parsed.remote,
                            &parsed.local,
                            transfer,
                            Some(progress.clone()),
                        )
                        .await
                } else {
                    client
                        .put_file_with_options_and_progress(
                            &parsed.remote,
                            &parsed.local,
                            transfer,
                            Some(progress.clone()),
                        )
                        .await
                };
                let _ = progress.finish().await;
                let _ = client.close().await;
                result
            });
            match result {
                Ok(()) => CliResponse::success(""),
                Err(err) => CliResponse::failure(format!("{err}\n"), 1),
            }
        }
        Err(err) => super::super::help::parse_error_response("put-help.txt", &err),
    }
}
