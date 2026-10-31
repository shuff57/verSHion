//! The licence history of what an item OFFERS (worker/src/pool/publish.ts `OFFERED` / `offerSync`): one `content_licences` row per (item, content,
//! licence), open while the item offers that content under that licence publicly (unlisted, held or removed offers nothing). Run at a publish
//! (end of its batch), a visibility change, and the hourly reconcile.
use rusqlite::{Connection, params};

/// SQL on item row `i`: it is OFFERED to the pool now (public, live, not held): only then does a licence count.
pub fn offered_sql(i: &str) -> String {
    format!(
        "({i}.visibility = 'public' AND {i}.removed_at IS NULL AND {i}.purged_at IS NULL AND {i}.current_publish_id IS NOT NULL AND NOT EXISTS (SELECT 1 FROM item_holds oh WHERE oh.item_id = {i}.id))"
    )
}

/// Closes the item's open licence rows it no longer offers and re-opens (never duplicates) the ones it offers. `publish_id`: only while it is the
/// item's current publish.
pub fn offer_sync(conn: &Connection, item_id: &str, now: i64, publish_id: Option<&str>) -> rusqlite::Result<()> {
    let cond = if publish_id.is_none() { "?3 IS NULL" } else { "EXISTS (SELECT 1 FROM items gi WHERE gi.id = ?2 AND gi.current_publish_id = ?3)" };
    conn.execute(
        &format!(
            "UPDATE content_licences SET until_at = ?1 WHERE item_id = ?2 AND until_at IS NULL AND {cond}
                AND NOT EXISTS (SELECT 1 FROM items i JOIN item_offer o ON o.item_id = i.id WHERE i.id = ?2 AND o.ckey = content_licences.ckey AND i.license = content_licences.license AND {})",
            offered_sql("i")
        ),
        params![now, item_id, publish_id],
    )?;
    conn.execute(
        &format!(
            "INSERT INTO content_licences (item_id, ckey, license, from_at) SELECT i.id, o.ckey, i.license, ?1 FROM items i JOIN item_offer o ON o.item_id = i.id
              WHERE i.id = ?2 AND {} AND {cond}
             ON CONFLICT (item_id, ckey, license) DO UPDATE SET until_at = NULL WHERE content_licences.until_at IS NOT NULL",
            offered_sql("i")
        ),
        params![now, item_id, publish_id],
    )?;
    Ok(())
}
