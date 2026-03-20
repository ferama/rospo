mod interactive;
mod session;
mod transport;
mod types;

use crate::logging::{Logger, GREEN};

pub use session::Session;
pub use transport::{fetch_server_public_key, load_secret_key};
pub use types::{ClientOptions, ForwardedTcpIp, JumpHostOptions, Status};

pub(crate) const LOG: Logger = Logger::new("[SSHC] ", GREEN);
