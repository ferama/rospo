use crate::cli::app::{GetArgs, PutArgs, TunArgs, TunCommand};
use crate::cli::parse::client_options_from_cli;
use crate::cli::CliResponse;
use crate::sftp::{self, TransferOptions};
use crate::tunnel;
use crate::utils::new_endpoint;

use super::transfer_progress::ProgressManager;

pub(crate) fn tun_command(args: TunArgs) -> CliResponse {
    let (ssh_args, local, remote, forward, server) = match args.command {
        TunCommand::Forward(args) => (args.ssh, args.local, args.remote, true, args.server),
        TunCommand::Reverse(args) => (args.ssh, args.local, args.remote, false, args.server),
    };
    let options = match client_options_from_cli(&server, &ssh_args) {
        Ok(options) => options,
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };
    let local = match new_endpoint(&local) {
        Ok(endpoint) => endpoint,
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };
    let remote = match new_endpoint(&remote) {
        Ok(endpoint) => endpoint,
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };

    let runtime = match tokio::runtime::Builder::new_multi_thread().worker_threads(2).enable_all().build() {
        Ok(runtime) => runtime,
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };

    let result = runtime.block_on(async move {
        if forward {
            tunnel::run_forward(options, local, remote).await
        } else {
            tunnel::run_reverse(options, local, remote).await
        }
    });

    match result {
        Ok(()) => CliResponse::success(""),
        Err(err) => CliResponse::failure(format!("{err}\n"), 1),
    }
}

pub(crate) fn get_command(args: GetArgs) -> CliResponse {
    let options = match client_options_from_cli(&args.server, &args.ssh) {
        Ok(options) => options,
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };
    let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };
    let result = runtime.block_on(async move {
        let progress = std::sync::Arc::new(ProgressManager::download());
        let mut client = sftp::Client::connect(options).await?;
        let transfer = TransferOptions::new(args.max_workers, args.concurrent_downloads);
        let result = if args.recursive {
            client
                .get_recursive_with_options_and_progress(
                    &args.remote,
                    args.local.as_deref().unwrap_or(""),
                    transfer,
                    Some(progress.clone()),
                )
                .await
        } else {
            client
                .get_file_with_options_and_progress(
                    &args.remote,
                    args.local.as_deref().unwrap_or(""),
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

pub(crate) fn put_command(args: PutArgs) -> CliResponse {
    let options = match client_options_from_cli(&args.server, &args.ssh) {
        Ok(options) => options,
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };
    let runtime = match tokio::runtime::Builder::new_current_thread().enable_all().build() {
        Ok(runtime) => runtime,
        Err(err) => return CliResponse::failure(format!("{err}\n"), 1),
    };
    let result = runtime.block_on(async move {
        let progress = std::sync::Arc::new(ProgressManager::upload());
        let mut client = sftp::Client::connect(options).await?;
        let transfer = TransferOptions::new(args.max_workers, args.concurrent_uploads);
        let result = if args.recursive {
            client
                .put_recursive_with_options_and_progress(
                    args.remote.as_deref().unwrap_or(""),
                    &args.local,
                    transfer,
                    Some(progress.clone()),
                )
                .await
        } else {
            client
                .put_file_with_options_and_progress(
                    args.remote.as_deref().unwrap_or(""),
                    &args.local,
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
