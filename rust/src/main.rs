use clap::Parser;

#[cfg(not(windows))]
fn main() {
    let cli = rospo::cli::Cli::parse();
    std::process::exit(rospo::cli::run_cli(cli));
}

#[cfg(windows)]
fn main() {
    let args = std::env::args_os().collect::<Vec<_>>();
    match rospo::windows_service::try_run(args.clone()) {
        Ok(true) => {}
        Ok(false) => std::process::exit(rospo::cli::run(args)),
        Err(err) => {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }
}
