use clap::{Args, Parser, Subcommand};

use crate::sftp;

use super::VERSION;

#[derive(Debug, Parser)]
#[command(name = "rospo", version = VERSION, disable_help_subcommand = true)]
pub struct Cli {
    /// If set disable all logs.
    #[arg(short = 'q', long = "quiet", global = true)]
    pub(crate) quiet: bool,

    #[command(subcommand)]
    pub(crate) command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Run services from a YAML configuration file.
    Run(RunArgs),
    /// Print a compatible configuration template.
    Template,
    /// Generate a P-521 SSH identity.
    Keygen(KeygenArgs),
    /// Grab a server public key and append it to known_hosts.
    Grabpubkey(GrabPubkeyArgs),
    /// Open a remote shell or execute a remote command.
    Shell(ShellArgs),
    /// Download files from the remote host.
    Get(GetArgs),
    /// Upload files to the remote host.
    Put(PutArgs),
    /// Start a local SOCKS4/5 proxy over SSH.
    #[command(name = "socks-proxy")]
    SocksProxy(SocksProxyArgs),
    /// Start a local DNS proxy over SSH.
    #[command(name = "dns-proxy")]
    DnsProxy(DnsProxyArgs),
    /// Start a forward or reverse tunnel.
    Tun(TunArgs),
    /// Run the embedded SSH server.
    Sshd(SshdArgs),
    /// Start a reverse shell by combining sshd and a reverse tunnel.
    Revshell(RevshellArgs),
    /// Show command help.
    Help(HelpArgs),
}

#[derive(Debug, Args)]
pub(crate) struct HelpArgs {
    /// Command to describe.
    pub(crate) command: Option<String>,

    /// Nested subcommand to describe.
    pub(crate) subcommand: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct RunArgs {
    /// YAML configuration file to execute.
    pub(crate) config: String,
}

#[derive(Debug, Args)]
pub(crate) struct KeygenArgs {
    /// Store the generated keys on disk.
    #[arg(short = 's', long = "store")]
    pub(crate) store: bool,

    /// Output directory for generated key files.
    #[arg(short = 'p', long = "path", default_value = ".")]
    pub(crate) path: String,

    /// Base file name for generated key files.
    #[arg(short = 'n', long = "name", default_value = "identity")]
    pub(crate) name: String,
}

#[derive(Debug, Args)]
pub(crate) struct GrabPubkeyArgs {
    /// Known hosts file path.
    #[arg(short = 'k', long = "known-hosts", default_value = "~/.ssh/known_hosts")]
    pub(crate) known_hosts: String,

    /// SSH server in [user@]host[:port] form.
    pub(crate) server: String,
}

#[derive(Debug, Args, Clone)]
pub(crate) struct SshClientArgs {
    /// Do not print the SSH banner.
    #[arg(short = 'b', long = "disable-banner")]
    pub(crate) disable_banner: bool,

    /// Skip known_hosts verification.
    #[arg(short = 'i', long = "insecure")]
    pub(crate) insecure: bool,

    /// Jump host in [user@]host[:port] form.
    #[arg(short = 'j', long = "jump-host")]
    pub(crate) jump_host: Option<String>,

    /// SSH private key path.
    #[arg(short = 's', long = "user-identity")]
    pub(crate) user_identity: Option<String>,

    /// Known hosts file path.
    #[arg(short = 'k', long = "known-hosts")]
    pub(crate) known_hosts: Option<String>,

    /// SSH password.
    #[arg(short = 'p', long = "password")]
    pub(crate) password: Option<String>,
}

#[derive(Debug, Args)]
#[command(trailing_var_arg = true)]
pub(crate) struct ShellArgs {
    #[command(flatten)]
    pub(crate) ssh: SshClientArgs,

    /// SSH server in [user@]host[:port] form.
    pub(crate) server: String,

    /// Remote command to execute instead of an interactive shell.
    #[arg(allow_hyphen_values = true)]
    pub(crate) command: Vec<String>,
}

#[derive(Debug, Args)]
pub(crate) struct GetArgs {
    #[command(flatten)]
    pub(crate) ssh: SshClientArgs,

    /// Parallel workers per file.
    #[arg(short = 'w', long = "max-workers", default_value_t = sftp::DEFAULT_DOWNLOAD_MAX_WORKERS)]
    pub(crate) max_workers: usize,

    /// Concurrent recursive downloads.
    #[arg(short = 'c', long = "concurrent-downloads", default_value_t = sftp::DEFAULT_CONCURRENT_DOWNLOADS)]
    pub(crate) concurrent_downloads: usize,

    /// Copy directories recursively.
    #[arg(short = 'r', long = "recursive")]
    pub(crate) recursive: bool,

    /// SSH server in [user@]host[:port] form.
    pub(crate) server: String,

    /// Remote path to download.
    pub(crate) remote: String,

    /// Local destination path.
    pub(crate) local: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct PutArgs {
    #[command(flatten)]
    pub(crate) ssh: SshClientArgs,

    /// Parallel workers per file.
    #[arg(short = 'w', long = "max-workers", default_value_t = sftp::DEFAULT_UPLOAD_MAX_WORKERS)]
    pub(crate) max_workers: usize,

    /// Concurrent recursive uploads.
    #[arg(short = 'c', long = "concurrent-uploads", default_value_t = sftp::DEFAULT_CONCURRENT_UPLOADS)]
    pub(crate) concurrent_uploads: usize,

    /// Copy directories recursively.
    #[arg(short = 'r', long = "recursive")]
    pub(crate) recursive: bool,

    /// SSH server in [user@]host[:port] form.
    pub(crate) server: String,

    /// Local path to upload.
    pub(crate) local: String,

    /// Remote destination path.
    pub(crate) remote: Option<String>,
}

#[derive(Debug, Args)]
pub(crate) struct SocksProxyArgs {
    #[command(flatten)]
    pub(crate) ssh: SshClientArgs,

    /// Local listen address.
    #[arg(short = 'l', long = "listen-address", default_value = "127.0.0.1:1080")]
    pub(crate) listen_address: String,

    /// SSH server in [user@]host[:port] form.
    pub(crate) server: String,
}

#[derive(Debug, Args)]
pub(crate) struct DnsProxyArgs {
    #[command(flatten)]
    pub(crate) ssh: SshClientArgs,

    /// Local listen address.
    #[arg(short = 'l', long = "listen-address", default_value = ":53")]
    pub(crate) listen_address: String,

    /// Remote DNS server.
    #[arg(short = 'd', long = "remote-dns-server", default_value = "1.1.1.1:53")]
    pub(crate) remote_dns_server: String,

    /// SSH server in [user@]host[:port] form.
    pub(crate) server: String,
}

#[derive(Debug, Args)]
pub(crate) struct TunArgs {
    #[command(subcommand)]
    pub(crate) command: TunCommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum TunCommand {
    /// Start a local forward tunnel.
    Forward(TunnelEndpointArgs),
    /// Start a remote reverse tunnel.
    Reverse(TunnelEndpointArgs),
}

#[derive(Debug, Args)]
pub(crate) struct TunnelEndpointArgs {
    #[command(flatten)]
    pub(crate) ssh: SshClientArgs,

    /// Local tunnel endpoint.
    #[arg(short = 'l', long = "local", default_value = "127.0.0.1:2222")]
    pub(crate) local: String,

    /// Remote tunnel endpoint.
    #[arg(short = 'r', long = "remote", default_value = "127.0.0.1:2222")]
    pub(crate) remote: String,

    /// SSH server in [user@]host[:port] form.
    pub(crate) server: String,
}

#[derive(Debug, Args, Clone)]
pub(crate) struct SshdSharedArgs {
    /// Authorized keys file or URL.
    #[arg(short = 'K', long = "sshd-authorized-keys", default_value = "./authorized_keys")]
    pub(crate) sshd_authorized_keys: String,

    /// SSH server listen address.
    #[arg(short = 'P', long = "sshd-listen-address", default_value = ":2222")]
    pub(crate) sshd_listen_address: String,

    /// SSH server private key path.
    #[arg(short = 'I', long = "sshd-key", default_value = "./server_key")]
    pub(crate) sshd_key: String,

    /// Disable SSH authentication.
    #[arg(short = 'T', long = "disable-auth")]
    pub(crate) disable_auth: bool,

    /// Disable the shell subsystem.
    #[arg(short = 'D', long = "disable-shell")]
    pub(crate) disable_shell: bool,

    /// Authorized password for password auth.
    #[arg(short = 'A', long = "sshd-authorized-password", default_value = "")]
    pub(crate) sshd_authorized_password: String,
}

#[derive(Debug, Args)]
pub(crate) struct SshdArgs {
    #[command(flatten)]
    pub(crate) sshd: SshdSharedArgs,
}

#[derive(Debug, Args)]
pub(crate) struct RevshellArgs {
    #[command(flatten)]
    pub(crate) ssh: SshClientArgs,

    #[command(flatten)]
    pub(crate) sshd: SshdSharedArgs,

    /// Remote listener endpoint.
    #[arg(short = 'r', long = "remote", default_value = "127.0.0.1:2222")]
    pub(crate) remote: String,

    /// SSH server in [user@]host[:port] form.
    pub(crate) server: String,
}
