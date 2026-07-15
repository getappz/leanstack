//! Coaching rule data model: the `CoachingRule` struct and the
//! `coaching-<id>.md` file format (parsing + serialization).

/// A coaching rule loaded from a `coaching-<id>.md` file.
#[derive(Debug)]
pub struct CoachingRule {
    pub id: String,
    pub title: String,
    pub body: String,
    pub applied_at: String,
}

/// Validate a rule id: non-empty, max 10 chars, starts with an ASCII
/// letter, remaining chars ASCII alphanumeric or `-`. Ported from
/// claude-view's `is_valid_pattern_id`.
pub(super) fn is_valid_rule_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 10
        && id.starts_with(|c: char| c.is_ascii_alphabetic())
        && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
}

/// Parse a single `coaching-<id>.md` file into a `CoachingRule`. Returns
/// `None` if the filename doesn't match the expected pattern, or if a
/// matching file has no valid `# Applied:` header — such files are skipped
/// entirely (not included with blank fields) so one bad file can never take
/// down the whole listing.
pub(super) fn parse_rule_file(path: &std::path::Path) -> Option<CoachingRule> {
    let content = std::fs::read_to_string(path).ok()?;
    let filename = path.file_stem()?.to_str()?;
    let id = filename.strip_prefix("coaching-")?.to_string();

    let mut title = String::new();
    let mut applied_at = String::new();
    let mut in_header = false;
    let mut header_done = false;
    let mut body_lines = Vec::new();

    for line in content.lines() {
        if line.starts_with("---") && !header_done {
            in_header = !in_header;
            if !in_header {
                header_done = true;
            }
            continue;
        }
        if in_header {
            if let Some(rest) = line.strip_prefix("# Pattern:") {
                if let Some(t) = rest.split_once('\u{2014}').map(|x| x.1) {
                    title = t.trim().to_string();
                }
            } else if let Some(rest) = line.strip_prefix("# Applied:") {
                applied_at = rest.trim().to_string();
            }
        } else if !line.is_empty() {
            body_lines.push(line);
        }
    }

    // Only return a rule if it has a valid # Applied: header (i.e., was
    // written by write_rule_file). Malformed/empty files are skipped.
    if applied_at.is_empty() {
        return None;
    }

    Some(CoachingRule {
        id,
        title,
        body: body_lines.join(" "),
        applied_at,
    })
}

pub(super) fn write_rule_file(
    dir: &std::path::Path,
    id: &str,
    title: &str,
    body: &str,
) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)?;
    let date = chrono::Local::now().date_naive();
    let content =
        format!("---\n# Pattern: {id} \u{2014} {title}\n# Applied: {date}\n---\n\n{body}\n");
    std::fs::write(dir.join(format!("coaching-{id}.md")), content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_valid_rule_id_accepts_short_alpha_start_ids() {
        assert!(is_valid_rule_id("hygiene"));
        assert!(is_valid_rule_id("a"));
        assert!(is_valid_rule_id("model-1"));
    }

    #[test]
    fn is_valid_rule_id_rejects_empty_too_long_numeric_start_or_bad_chars() {
        assert!(!is_valid_rule_id(""));
        assert!(!is_valid_rule_id("waytoolongforanid"));
        assert!(!is_valid_rule_id("1abc"));
        assert!(!is_valid_rule_id("has space"));
        assert!(!is_valid_rule_id("has_underscore"));
    }
}
