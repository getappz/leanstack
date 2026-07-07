use crate::config;
use std::path::Path;

static EMBEDDED_SKILL: &str = include_str!("skill.md");

pub struct Instructions {
    #[allow(dead_code)]
    pub mode: String,
    pub body: String,
}

const SKILL_URL: &str =
    "https://raw.githubusercontent.com/DietrichGebert/ponytail/main/skills/ponytail/SKILL.md";

pub fn skill_cache_path() -> std::path::PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join("agentflare")
        .join("ponytail")
        .join("SKILL.md")
}

pub fn download_skill() -> Result<String, String> {
    let resp = ureq::get(SKILL_URL)
        .timeout(std::time::Duration::from_secs(30))
        .call()
        .map_err(|e| format!("fetch failed: {e}"))?;
    if resp.status() != 200 {
        return Err(format!("HTTP {}", resp.status()));
    }
    let body = resp.into_string().map_err(|e| format!("read failed: {e}"))?;
    let path = skill_cache_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("mkdir: {e}"))?;
    }
    std::fs::write(&path, &body).map_err(|e| format!("write: {e}"))?;
    Ok(path.display().to_string())
}

pub fn build(mode: &str, skill_path: Option<&Path>) -> Instructions {
    let effective = config::normalize_persisted_mode(mode)
        .unwrap_or(config::DEFAULT_MODE);

    if crate::sub_skills::get(effective).is_some() {
        return Instructions {
            mode: effective.to_string(),
            body: format!(
                "PONYTAIL MODE ACTIVE — level: {effective}. Behavior defined by /ponytail-{effective} skill."
            ),
        };
    }

    let skill_body = if let Some(path) = skill_path {
        std::fs::read_to_string(path).unwrap_or_else(|_| EMBEDDED_SKILL.to_string())
    } else {
        std::fs::read_to_string(skill_cache_path()).unwrap_or_else(|_| EMBEDDED_SKILL.to_string())
    };

    let filtered = filter_skill_body(&skill_body, effective);

    Instructions {
        mode: effective.to_string(),
        body: filtered,
    }
}

pub fn filter_skill_body(body: &str, mode: &str) -> String {
    let effective = config::normalize_mode(mode).unwrap_or(config::DEFAULT_MODE);
    body.lines()
        .filter(|line| {
            if let Some(cap) = line.trim().strip_prefix("| **")
                && let Some(end) = cap.find("** |")
            {
                let label_mode = config::normalize_mode(&cap[..end]);
                if let Some(lm) = label_mode {
                    return lm == effective;
                }
            }
            if let Some(rest) = line.trim().strip_prefix("- ")
                && let Some(colon) = rest.find(':')
            {
                let label_mode = config::normalize_mode(rest[..colon].trim());
                if let Some(lm) = label_mode {
                    return lm == effective;
                }
            }
            true
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[allow(dead_code)]
pub fn fallback_instructions(mode: &str) -> String {
    let m = config::normalize_mode(mode).unwrap_or(config::DEFAULT_MODE);
    format!(
        "PONYTAIL MODE ACTIVE — level: {m}\n\n\
         You are a lazy senior developer. Lazy means efficient, not careless — \
         less work for the same result. The best code is the code never written.\n\n\
         Pick the first rung that holds:\n\n\
         1. Does this need to exist? (YAGNI)\n\
         2. Already in this codebase? Reuse it.\n\
         3. Stdlib? Use it.\n\
         4. Native platform feature? Use it.\n\
         5. Installed dependency? Use it.\n\
         6. One line? One line.\n\
         7. Only then: write the minimum.\n\n\
         Read and trace the flow first, then climb.\n\
         Bug fix = root cause. Fix the shared function once, not every caller.\n\n\
         Rules: no unrequested abstractions, no new deps, no boilerplate, \
         delete over add, shortest diff wins, edge-case-correct when same size, \
         mark shortcuts with `ponytail:` comment + ceiling + upgrade path.\n\n\
         NEVER invent APIs, functions, or variables that don't exist. Verify before using.\n\n\
         Not lazy: understanding the problem, validation at trust boundaries, \
         error handling that prevents data loss, security, accessibility. \
         One runnable check per non-trivial change.\n\n\
         Act the role, never label it. \
         One-line simplification markers: `ponytail: <skipped>, add when <condition>`. \
         Delete if longer than code.",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_generates_for_mode() {
        let f = fallback_instructions("full");
        assert!(f.contains("PONYTAIL MODE ACTIVE"));
        assert!(f.contains("Pick the first rung"));
        assert!(f.contains("root cause"));
    }

    #[test]
    fn build_uses_embedded_skill() {
        let ins = build("full", None);
        assert!(!ins.body.is_empty());
        assert_eq!(ins.mode, "full");
    }

    #[test]
    fn filter_keeps_non_mode_lines() {
        let input = "some rule\n| **lite** | lite only |\n| **full** | full only |\nother rule";
        let filtered = filter_skill_body(input, "full");
        assert!(filtered.contains("some rule"));
        assert!(filtered.contains("full only"));
        assert!(!filtered.contains("lite only"));
        assert!(filtered.contains("other rule"));
    }
}
