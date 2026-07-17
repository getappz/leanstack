use agentflare_store::{Error, Store};
use std::path::PathBuf;

pub fn store_path() -> PathBuf {
    crate::paths::home()
        .join(".agentflare")
        .join("agentflare.db")
}

/// Opens a fresh connection to the local store on every call -- deliberately
/// not cached behind a `OnceLock`. `crate::paths::home()` respects
/// `AGENTFLARE_HOME_OVERRIDE` (see `paths::test_support::with_temp_home`),
/// which tests flip per-call; a cached singleton would keep pointing at the
/// first test's home dir forever. Mirrors `memory::store::open()`'s same
/// per-call-open pattern for the same reason.
pub fn open() -> Result<Store, Error> {
    let path = store_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    Store::open_file(&path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::test_support::with_temp_home;

    #[test]
    fn open_and_close() {
        with_temp_home(|| {
            let store = open().unwrap();
            store.conn().execute_batch("SELECT 1").unwrap();
        });
    }

    #[test]
    fn each_call_sees_the_current_home_override() {
        with_temp_home(|| {
            let first = store_path();
            open().unwrap();
            assert!(first.exists());
        });
    }
}
