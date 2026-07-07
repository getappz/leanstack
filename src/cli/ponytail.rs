use clap::{Args, Subcommand};

#[derive(Subcommand)]
pub enum PonytailAction {
    Status,
    Set {
        mode: String,
    },
    Default {
        mode: String,
    },
    Off,
    Update,
    Review,
    Audit,
    Debt,
    Gain,
    Info,
    Hook {
        #[command(subcommand)]
        event: PonytailHookEvent,
    },
}

#[derive(Subcommand)]
pub enum PonytailHookEvent {
    SessionStart,
    SubagentStart,
    PromptSubmit,
    Statusline,
}

#[derive(Args)]
pub struct PonytailArgs {
    #[command(subcommand)]
    pub action: PonytailAction,
}

impl PonytailArgs {
    pub fn run(self) {
        match self.action {
            PonytailAction::Status => {
                let mode = crate::ponytail::active_mode().unwrap_or_else(crate::ponytail::default_mode);
                println!("{mode}");
            }
            PonytailAction::Set { mode } => {
                let normalized = crate::ponytail::normalize_config_mode(&mode)
                    .unwrap_or("full");
                crate::ponytail::set_active(normalized).unwrap_or_else(|e| {
                    eprintln!("error: {e}");
                    std::process::exit(1);
                });
                println!("{normalized}");
            }
            PonytailAction::Default { mode } => {
                crate::ponytail::set_default_mode(&mode).unwrap_or_else(|e| {
                    eprintln!("error: {e}");
                    std::process::exit(1);
                });
                crate::ponytail::set_active(&mode).ok();
                println!("default: {mode}");
            }
            PonytailAction::Off => {
                crate::ponytail::clear_active();
                println!("off");
            }
            PonytailAction::Update => {
                match crate::ponytail::download_skill() {
                    Ok(path) => println!("SKILL.md updated at {path}"),
                    Err(e) => {
                        eprintln!("update failed: {e}");
                        std::process::exit(1);
                    }
                }
            }
            PonytailAction::Review => {
                println!("{}", crate::ponytail::sub_skills::SKILL_REVIEW);
            }
            PonytailAction::Audit => {
                println!("{}", crate::ponytail::sub_skills::SKILL_AUDIT);
            }
            PonytailAction::Debt => {
                println!("{}", crate::ponytail::sub_skills::SKILL_DEBT);
            }
            PonytailAction::Gain => {
                println!("{}", crate::ponytail::sub_skills::SKILL_GAIN);
            }
            PonytailAction::Info => {
                println!("{}", crate::ponytail::sub_skills::SKILL_HELP);
            }
            PonytailAction::Hook { event } => match event {
                PonytailHookEvent::SessionStart => {
                    let mode = crate::ponytail::active_mode()
                        .unwrap_or_else(crate::ponytail::default_mode);
                    if mode == "off" {
                        crate::ponytail::state::clear_active();
                        println!("OK");
                        return;
                    }
                    crate::ponytail::set_active(&mode).ok();
                    let instructions = crate::ponytail::build_instructions(&mode, None);
                    let platform = crate::ponytail::detect_platform();
                    let output = crate::ponytail::format_hook_output(
                        "SessionStart",
                        &instructions.body,
                        &platform,
                    );
                    println!("{output}");
                }
                PonytailHookEvent::SubagentStart => {
                    let mode = crate::ponytail::active_mode()
                        .unwrap_or_else(crate::ponytail::default_mode);
                    if mode == "off" {
                        println!("OK");
                        return;
                    }
                    let instructions = crate::ponytail::build_instructions(&mode, None);
                    let platform = crate::ponytail::detect_platform();
                    let output = crate::ponytail::format_hook_output(
                        "SubagentStart",
                        &instructions.body,
                        &platform,
                    );
                    println!("{output}");
                }
                PonytailHookEvent::PromptSubmit => {
                    let mut input = String::new();
                    std::io::stdin().read_line(&mut input).ok();
                    if let Some(action) = crate::ponytail::detect_switch(&input) {
                        match action {
                            crate::ponytail::SwitchAction::SetMode(m) => {
                                crate::ponytail::set_active(&m).ok();
                            }
                            crate::ponytail::SwitchAction::SetDefault(m) => {
                                crate::ponytail::set_default_mode(&m).ok();
                                crate::ponytail::set_active(&m).ok();
                            }
                            crate::ponytail::SwitchAction::Off => {
                                crate::ponytail::clear_active();
                            }
                        }
                    }
                    println!("OK");
                }
                PonytailHookEvent::Statusline => {
                    let mode = crate::ponytail::active_mode()
                        .unwrap_or_else(crate::ponytail::default_mode);
                    if mode == "off" || mode.is_empty() {
                        return;
                    }
                    if mode == "full" {
                        print!("\x1b[38;5;108m[PONYTAIL]\x1b[0m");
                    } else {
                        let upper = mode.to_uppercase();
                        print!("\x1b[38;5;108m[PONYTAIL:{upper}]\x1b[0m");
                    }
                }
            },
        }
    }
}
