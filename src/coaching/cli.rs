//! CLI-facing presentation for `agentflare coaching {list,apply,remove}`.

use super::store::{self, MAX_RULES};

pub fn print_list() {
    let rules = store::list_rules();
    if rules.is_empty() {
        println!(
            "No coaching rules configured. Add one with `agentflare coaching apply <id> --title <title> --body <body>`."
        );
        return;
    }
    println!("agentflare coaching rules ({}/{MAX_RULES}):\n", rules.len());
    for r in &rules {
        println!("  {:<10} {}  (applied {})", r.id, r.title, r.applied_at);
        println!("    {}", r.body);
    }
}

pub fn cli_apply(id: &str, title: &str, body: &str) {
    match store::apply_rule(id, title, body) {
        Ok(rule) => println!("Applied coaching rule '{}': {}", rule.id, rule.title),
        Err(e) => {
            eprintln!("agentflare coaching apply: {e}");
            std::process::exit(1);
        }
    }
}

pub fn cli_remove(id: &str) {
    match store::remove_rule(id) {
        Ok(()) => println!("Removed coaching rule '{id}'."),
        Err(e) => {
            eprintln!("agentflare coaching remove: {e}");
            std::process::exit(1);
        }
    }
}
