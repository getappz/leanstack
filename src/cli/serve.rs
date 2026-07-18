use clap::Args;

/// Serve the read-only agentflare dashboard.
#[derive(Args)]
pub struct ServeArgs {
    /// TCP port (0 = auto).
    #[arg(long, default_value = "0")]
    pub port: u16,
    /// Interface to bind ("0.0.0.0" shares with your LAN).
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,
    /// Open the dashboard in your browser once the server is up.
    #[arg(long)]
    pub open: bool,
}

impl ServeArgs {
    pub fn run(self) {
        crate::dashboard::serve(&self.host, self.port, self.open);
    }
}
