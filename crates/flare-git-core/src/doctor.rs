use std::path::Path;

use crate::shell::{run_in as run_git_in, run_in_ok};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct LaneHealth {
    pub name: String,
    pub path: String,
    pub sequence_id: Option<i64>,
    pub flags: Vec<HealthFlag>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum HealthFlag {
    Dirty,
    Stale { days: u64 },
    MissingWorktree,
    DuplicateBranch { other_paths: Vec<String> },
    MissingUpstream,
    Orphaned { item_state: String },
    Zombie,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct DoctorReport {
    pub lanes: Vec<LaneHealth>,
    pub violations: Vec<String>,
    pub summary: Summary,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Summary {
    pub total: usize,
    pub flagged: usize,
    pub dirty: usize,
    pub stale: usize,
    pub missing_worktree: usize,
    pub duplicate_branch: usize,
    pub missing_upstream: usize,
    pub orphaned: usize,
    pub zombie: usize,
    pub clean: usize,
}

pub fn scan(repo_root: &Path, staleness_days: u64) -> DoctorReport {
    let mut lanes = Vec::new();
    let mut violations = Vec::new();
    let mut branch_lanes: std::collections::BTreeMap<String, Vec<(String, String)>> =
        std::collections::BTreeMap::new();

    let worktrees = list_worktrees(repo_root);
    let git_worktree_entries = parse_worktree_list(&worktrees);

    for entry in &git_worktree_entries {
        let path = Path::new(&entry.path);
        let name = entry
            .branch
            .as_deref()
            .or_else(|| path.file_name().and_then(|n| n.to_str()))
            .unwrap_or("unknown")
            .to_string();

        let mut flags: Vec<HealthFlag> = Vec::new();
        let sequence_id = name
            .strip_prefix("task/")
            .and_then(|s| s.parse::<i64>().ok());

        if let Some(ref branch) = entry.branch {
            branch_lanes
                .entry(branch.clone())
                .or_default()
                .push((name.clone(), entry.path.clone()));

            if !has_upstream(repo_root, branch) {
                flags.push(HealthFlag::MissingUpstream);
            }
        }

        if !path.exists() {
            flags.push(HealthFlag::MissingWorktree);
            lanes.push(LaneHealth {
                name: name.clone(),
                path: entry.path.clone(),
                sequence_id,
                flags,
            });
            continue;
        }

        if is_dirty(path) {
            flags.push(HealthFlag::Dirty);
        }

        flags.push(HealthFlag::Stale { days: staleness_days });

        lanes.push(LaneHealth {
            name,
            path: entry.path.clone(),
            sequence_id,
            flags,
        });
    }

    for (branch, instances) in &branch_lanes {
        if instances.len() > 1 {
            let other_paths: Vec<String> =
                instances.iter().map(|(_, p)| p.clone()).collect();
            for (inst_name, _inst_path) in instances {
                if let Some(lane) = lanes.iter_mut().find(|l| l.name == *inst_name) {
                    lane.flags
                        .push(HealthFlag::DuplicateBranch { other_paths: other_paths.clone() });
                }
            }
            violations.push(format!(
                "duplicate-branch: '{}' in {} worktrees",
                branch,
                instances.len()
            ));
        }
    }

    let total = lanes.len();
    let flagged = lanes.iter().filter(|l| !l.flags.is_empty()).count();
    let clean = lanes.iter().filter(|l| l.flags.is_empty()).count();
    let dirty_count = lanes
        .iter()
        .filter(|l| l.flags.iter().any(|f| matches!(f, HealthFlag::Dirty)))
        .count();
    let stale_count = lanes
        .iter()
        .filter(|l| l.flags.iter().any(|f| matches!(f, HealthFlag::Stale { .. })))
        .count();
    let missing_wt_count = lanes
        .iter()
        .filter(|l| l.flags.iter().any(|f| matches!(f, HealthFlag::MissingWorktree)))
        .count();
    let dup_count = lanes
        .iter()
        .filter(|l| l.flags.iter().any(|f| matches!(f, HealthFlag::DuplicateBranch { .. })))
        .count();
    let missing_up_count = lanes
        .iter()
        .filter(|l| l.flags.iter().any(|f| matches!(f, HealthFlag::MissingUpstream)))
        .count();
    let orphaned_count = lanes
        .iter()
        .filter(|l| l.flags.iter().any(|f| matches!(f, HealthFlag::Orphaned { .. })))
        .count();
    let zombie_count = lanes
        .iter()
        .filter(|l| l.flags.iter().any(|f| matches!(f, HealthFlag::Zombie)))
        .count();

    if dirty_count > 5 {
        violations.push(format!("{} dirty worktrees (threshold: >5)", dirty_count));
    }
    if stale_count > 3 {
        violations.push(format!(
            "{} stale worktrees (threshold: >3)",
            stale_count
        ));
    }

    DoctorReport {
        lanes,
        violations,
        summary: Summary {
            total,
            flagged,
            dirty: dirty_count,
            stale: stale_count,
            missing_worktree: missing_wt_count,
            duplicate_branch: dup_count,
            missing_upstream: missing_up_count,
            orphaned: orphaned_count,
            zombie: zombie_count,
            clean,
        },
    }
}

pub fn reclaim(
    repo_root: &Path,
    report: &DoctorReport,
    force: bool,
) -> Vec<String> {
    let mut reclaimed = Vec::new();
    for lane in &report.lanes {
        let has_dirty = lane.flags.iter().any(|f| matches!(f, HealthFlag::Dirty));
        if has_dirty && !force {
            continue;
        }
        let is_safe = lane.flags.iter().all(|f| match f {
            HealthFlag::Dirty => force,
            HealthFlag::Stale { .. } => true,
            HealthFlag::MissingWorktree => true,
            HealthFlag::DuplicateBranch { .. } => true,
            HealthFlag::MissingUpstream => true,
            HealthFlag::Orphaned { .. } => true,
            HealthFlag::Zombie => true,
        });
        if !is_safe {
            continue;
        }
        let path = Path::new(&lane.path);
        if path.exists() {
            if let Err(e) = crate::snapshot::snapshot_before(
                repo_root,
                &format!("doctor reclaim {}", lane.name),
            ) {
                eprintln!("doctor: snapshot failed before reclaiming {}: {}", lane.name, e);
                continue;
            }
            if std::fs::remove_dir_all(path).is_ok() {
                let _ = crate::shell::run_in(repo_root, &["worktree", "prune"]);
                reclaimed.push(lane.name.clone());
            }
        } else {
            reclaimed.push(lane.name.clone());
        }
    }
    reclaimed
}

pub fn format_text(report: &DoctorReport) -> String {
    let mut out = String::new();
    out.push_str("=== flare doctor ===\n\n");
    for lane in &report.lanes {
        out.push_str(&format!("  {}  ({})\n", lane.name, lane.path));
        for flag in &lane.flags {
            let line = match flag {
                HealthFlag::Dirty => "    ⚠ dirty: uncommitted changes".into(),
                HealthFlag::Stale { days } => format!("    ⏳ stale: untouched >{}d", days),
                HealthFlag::MissingWorktree => "    ✗ missing-worktree: path gone".into(),
                HealthFlag::DuplicateBranch { other_paths } => {
                    format!("    ⚠ duplicate-branch: also at {}", other_paths.join(", "))
                }
                HealthFlag::MissingUpstream => "    ⚠ missing-upstream: no remote".into(),
                HealthFlag::Orphaned { item_state } => {
                    format!("    🗑 orphaned: item {item_state}")
                }
                HealthFlag::Zombie => "    💀 zombie: session pid dead".into(),
            };
            out.push_str(&line);
            out.push('\n');
        }
    }
    out.push('\n');
    out.push_str(&format!("Summary: {} total, {} flagged, {} clean\n", report.summary.total, report.summary.flagged, report.summary.clean));
    if !report.violations.is_empty() {
        out.push_str("Violations:\n");
        for v in &report.violations {
            out.push_str(&format!("  - {v}\n"));
        }
    }
    out
}

pub fn format_json(report: &DoctorReport) -> String {
    serde_json::to_string_pretty(report).unwrap_or_else(|_| "{}".into())
}

pub fn format_markdown(report: &DoctorReport) -> String {
    let mut out = String::new();
    out.push_str("# flare doctor\n\n");
    out.push_str(&format!(
        "| Lane | Path | Flags |\n|------|------|-------|\n"
    ));
    for lane in &report.lanes {
        let flag_str = lane
            .flags
            .iter()
            .map(|f| match f {
                HealthFlag::Dirty => "dirty",
                HealthFlag::Stale { .. } => "stale",
                HealthFlag::MissingWorktree => "missing-worktree",
                HealthFlag::DuplicateBranch { .. } => "duplicate-branch",
                HealthFlag::MissingUpstream => "missing-upstream",
                HealthFlag::Orphaned { .. } => "orphaned",
                HealthFlag::Zombie => "zombie",
            })
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("| {} | {} | {} |\n", lane.name, lane.path, flag_str));
    }
    out.push('\n');
    out.push_str(&format!(
        "**Summary:** {} total, {} flagged, {} clean\n\n",
        report.summary.total, report.summary.flagged, report.summary.clean
    ));
    if !report.violations.is_empty() {
        out.push_str("**Violations:**\n");
        for v in &report.violations {
            out.push_str(&format!("- {v}\n"));
        }
    }
    out
}

struct WorktreeEntry {
    path: String,
    branch: Option<String>,
}

fn list_worktrees(repo_root: &Path) -> String {
    run_git_in(repo_root, &["worktree", "list", "--porcelain"]).unwrap_or_default()
}

fn parse_worktree_list(output: &str) -> Vec<WorktreeEntry> {
    let mut entries = Vec::new();
    let mut current_path: Option<String> = None;
    let mut current_branch: Option<String> = None;
    for line in output.lines() {
        if line.trim().is_empty() {
            if let Some(path) = current_path.take() {
                entries.push(WorktreeEntry {
                    path,
                    branch: current_branch.take(),
                });
            }
        } else if let Some(path) = line.strip_prefix("worktree ") {
            if let Some(prev_path) = current_path.take() {
                entries.push(WorktreeEntry {
                    path: prev_path,
                    branch: current_branch.take(),
                });
            }
            current_path = Some(path.to_string());
        } else if let Some(branch) = line.strip_prefix("branch refs/heads/") {
            current_branch = Some(branch.to_string());
        } else if let Some(rev) = line.strip_prefix("HEAD ") {
            if current_branch.is_none() {
                current_branch = Some(format!("detached at {rev}"));
            }
        }
    }
    if let Some(path) = current_path.take() {
        entries.push(WorktreeEntry {
            path,
            branch: current_branch.take(),
        });
    }
    entries
}

fn is_dirty(path: &Path) -> bool {
    !run_in_ok(path, &["status", "--porcelain"])
}

fn has_upstream(repo_root: &Path, branch: &str) -> bool {
    run_in_ok(repo_root, &["rev-parse", "--abbrev-ref", &format!("{branch}@{{upstream}}")])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_worktree_list_two_entries_with_branches() {
        let output = "worktree /repo1\nHEAD a\nbranch refs/heads/main\n\nworktree /repo2\nHEAD b\nbranch refs/heads/feature\n";
        let entries = parse_worktree_list(output);
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].branch.as_deref(), Some("main"));
        assert_eq!(entries[1].branch.as_deref(), Some("feature"));
    }

    #[test]
    fn parse_worktree_list_detached() {
        let output = "worktree /repo\nHEAD abc\n";
        let entries = parse_worktree_list(output);
        assert_eq!(entries.len(), 1);
        assert!(entries[0].branch.as_deref().unwrap_or("").contains("detached"));
    }

    #[test]
    fn parse_worktree_list_empty() {
        assert!(parse_worktree_list("").is_empty());
    }

    #[test]
    fn scan_returns_summary() {
        let report = scan(Path::new("/nonexistent"), 14);
        assert_eq!(report.summary.total, 0);
    }
}
