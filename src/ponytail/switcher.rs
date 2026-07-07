use crate::ponytail::config;

pub enum SwitchAction {
    SetMode(String),
    SetDefault(String),
    Off,
}

pub fn detect(input: &str) -> Option<SwitchAction> {
    let prompt = input.trim().to_lowercase();

    if config::is_deactivation(&prompt) {
        return Some(SwitchAction::Off);
    }

    let cmd = prompt
        .strip_prefix("/ponytail")
        .or_else(|| prompt.strip_prefix("@ponytail"))
        .or_else(|| prompt.strip_prefix("$ponytail"))?;

    let parts: Vec<&str> = cmd.split_whitespace().collect();
    let sub = parts.first().copied().unwrap_or("");
    let arg = parts.get(1).copied().unwrap_or("");

    if sub.is_empty() || sub == "lite" || sub == "full" || sub == "ultra" {
        let mode = if sub.is_empty() { "full" } else { sub };
        config::normalize_config_mode(mode)?;
        return Some(SwitchAction::SetMode(mode.to_string()));
    }

    match sub {
        "off" => Some(SwitchAction::Off),
        "default" => {
            let dmode = arg;
            if dmode.is_empty() {
                return None;
            }
            config::normalize_config_mode(dmode)?;
            Some(SwitchAction::SetDefault(dmode.to_string()))
        }
        _ => None,
    }
}
