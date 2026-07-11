use clap::Args;

/// Launch an agent through mise (so all mise-managed tools are on PATH for the
/// session and anything it spawns) with wrangler-style `.dev.vars` env vars
/// injected. Example: `agentflare run claude-code --env staging`.
///
/// With `--print`, run the agent non-interactively on the given prompt and
/// write its reply to stdout (for scripts, cron, and orchestrators).
#[derive(Args)]
pub struct RunArgs {
    /// Agent to launch (e.g. claude-code).
    pub agent: String,
    /// Env stage: load `.dev.vars.<stage>` instead of `.dev.vars` (replaces it).
    #[arg(long)]
    pub env: Option<String>,
    #[arg(long)]
    pub model: Option<String>,
    #[arg(long)]
    pub mode: Option<String>,
    /// Run non-interactively on this prompt and print the reply to stdout.
    #[arg(long, value_name = "PROMPT")]
    pub print: Option<String>,
    /// Timeout (seconds) for `--print` before the agent is killed.
    #[arg(long, default_value_t = 120)]
    pub timeout: u64,
    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    pub args: Vec<String>,
}

impl RunArgs {
    pub fn run(self) {
        if let Some(prompt) = self.print.as_deref() {
            let code = crate::agents::cli_run_headless(
                &self.agent,
                prompt,
                std::time::Duration::from_secs(self.timeout),
            );
            std::process::exit(code);
        }
        crate::agents::cli_run(
            &self.agent,
            self.env.as_deref(),
            self.model.as_deref(),
            self.mode.as_deref(),
            &self.args,
        );
    }
}
