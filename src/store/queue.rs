//! The queue: what arrived and has not been opened, read, and taken back.
//! Kept beside the store rather than in it, which is long enough (1.32,
//! when a desk's Mark these read joined it).

use super::*;

impl Store {
    /// What arrived and has not been opened, oldest first: the order things
    /// came in is the order to read them in. Every insert joins it and every
    /// open leaves it, so it is the unread set with an order and nothing more.
    pub fn queue(&self, limit: usize) -> Result<Vec<Doc>> {
        let conn = self.conn.lock().unwrap();
        let rows = conn
            .prepare(&format!(
                "SELECT {DOC_COLS} {HEAD_FROM} WHERE d.unread = 1 ORDER BY d.received_at, d.rowid LIMIT ?1"
            ))?
            .query_map(params![limit as i64], row_to_doc)?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    /// The newest documents still unread, newest first: Home's Arrived,
    /// which reads what came last, where the queue reads oldest first.
    pub fn newest_unread(&self, limit: usize) -> Result<Vec<Doc>> {
        let conn = self.conn.lock().unwrap();
        let rows = conn
            .prepare(&format!(
                "SELECT {DOC_COLS} {HEAD_FROM} WHERE d.unread = 1 ORDER BY d.received_at DESC, d.rowid DESC LIMIT ?1"
            ))?
            .query_map(params![limit as i64], row_to_doc)?
            .collect::<std::result::Result<_, _>>()?;
        Ok(rows)
    }

    /// How many are on the queue: what the bar says, however many rows the
    /// page was sent.
    pub fn waiting(&self) -> Result<i64> {
        let conn = self.conn.lock().unwrap();
        Ok(
            conn.query_row("SELECT COUNT(*) FROM head_docs WHERE unread = 1", [], |r| {
                r.get(0)
            })?,
        )
    }

    /// A document was opened. True when this is what took it off the queue.
    pub fn mark_read(&self, id: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        Ok(conn.execute(
            "UPDATE docs SET unread = 0 WHERE id = ?1 AND unread = 1",
            params![id],
        )? > 0)
    }

    /// Everything waiting, marked read without being opened. Returns what
    /// left the queue, so every open tab can take the same rows off.
    pub fn mark_all_read(&self) -> Result<Vec<String>> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let ids: Vec<String> = tx
            .prepare("SELECT id FROM head_docs WHERE unread = 1")?
            .query_map([], |r| r.get(0))?
            .collect::<std::result::Result<_, _>>()?;
        tx.execute("UPDATE docs SET unread = 0 WHERE unread = 1", [])?;
        tx.commit()?;
        Ok(ids)
    }

    /// Some of what waits, marked read without being opened: a desk's own
    /// (its head's Mark these read, 1.32), leaving every other desk's
    /// waiting. Returns the ones that were waiting; `mark_unread` takes them
    /// back.
    pub fn mark_ids_read(&self, ids: &[String]) -> Result<Vec<String>> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let mut read = Vec::new();
        for id in ids {
            if tx.execute(
                "UPDATE docs SET unread = 0 WHERE id = ?1 AND unread = 1",
                params![id],
            )? > 0
            {
                read.push(id.clone());
            }
        }
        tx.commit()?;
        Ok(read)
    }

    /// Mark all read, taken back: the ids it returned go back on the queue,
    /// except one removed since, which has no row to wait in. Returns the
    /// ones that went back.
    pub fn mark_unread(&self, ids: &[String]) -> Result<Vec<String>> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let mut back = Vec::new();
        for id in ids {
            if tx.execute(
                "UPDATE docs SET unread = 1 WHERE id = ?1 AND unread = 0 AND deleted_at = 0",
                params![id],
            )? > 0
            {
                back.push(id.clone());
            }
        }
        tx.commit()?;
        Ok(back)
    }
}
