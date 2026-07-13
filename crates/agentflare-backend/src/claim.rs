//! Item claim lease — a thin wrapper over agentflare-db-kit's generic
//! `ClaimLedger`, keyed by `item_id`. Pure lease primitive, no item-state
//! knowledge; `item::claim`/`item::claim_done` compose this with
//! `item::update_state` to make claiming actually mean something.
use db_kit::claim::ClaimLedger;
use rusqlite::Connection;

pub use db_kit::claim::Acquire;

const LEDGER: ClaimLedger = ClaimLedger::new("item_claims", &["item_id"]);

pub fn acquire(
    conn: &Connection,
    item_id: &str,
    owner: &str,
    now: i64,
    ttl_secs: i64,
) -> rusqlite::Result<Acquire> {
    LEDGER.acquire(conn, &[item_id], owner, now, ttl_secs)
}

pub fn heartbeat(
    conn: &Connection,
    item_id: &str,
    owner: &str,
    now: i64,
) -> rusqlite::Result<bool> {
    LEDGER.heartbeat(conn, &[item_id], owner, now)
}

pub fn release(conn: &Connection, item_id: &str, owner: &str) -> rusqlite::Result<bool> {
    LEDGER.release(conn, &[item_id], owner)
}

pub fn done(conn: &Connection, item_id: &str, owner: &str, now: i64) -> rusqlite::Result<bool> {
    LEDGER.done(conn, &[item_id], owner, now)
}
