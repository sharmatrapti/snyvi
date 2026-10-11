//! What a send needs from the library beyond the file itself: the hook's
//! draft taken over by the agent's own send, and the workflow a conversation
//! named. Beside the store rather than in it, which is long enough (1.33).

use super::*;

/// Who took a draft over, and under what: the agent's send, as `promote`
/// writes it onto the draft's row.
pub struct Promotion<'a> {
    pub origin: &'a str,
    pub sender: &'a str,
    pub session: &'a str,
    pub title: &'a str,
    pub workflow_key: &'a str,
    pub workflow_title: &'a str,
}

impl Store {
    /// The agent sent what the hook had already saved, byte for byte: the
    /// draft becomes the send. It waits, says who sent it and in which
    /// workflow, takes the title the agent gave, and moves to now -- so it is
    /// the arrival the agent tells the reader about, at the top of the lists,
    /// rather than an `existing` nobody was told of. Before 1.33 the send was
    /// dropped as a duplicate and the draft kept its H1 and its session.
    pub fn promote(&self, id: &str, by: &Promotion) -> Result<Doc> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        let (project_id, source_path): (i64, Option<String>) = tx.query_row(
            "SELECT project_id, source_path FROM docs WHERE id = ?1",
            params![id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        tx.execute(
            "INSERT INTO workflows(project_id, key, title, created_at) VALUES(?1, ?2, ?3, ?4)
             ON CONFLICT(project_id, key) DO NOTHING",
            params![project_id, by.workflow_key, by.workflow_title, now()],
        )?;
        tx.execute(
            "UPDATE docs SET unread = 1, origin = ?2, title = ?3, received_at = ?4,
                 sender = CASE WHEN ?5 = '' THEN sender ELSE ?5 END,
                 session = CASE WHEN ?6 = '' THEN session ELSE ?6 END,
                 workflow_id = (SELECT id FROM workflows WHERE project_id = ?7 AND key = ?8)
             WHERE id = ?1",
            params![
                id,
                by.origin,
                by.title,
                now(),
                by.sender,
                by.session,
                project_id,
                by.workflow_key
            ],
        )?;
        tx.execute(
            "UPDATE docs_fts SET title = ?2 WHERE rowid = (SELECT rowid FROM docs WHERE id = ?1)",
            params![id, by.title],
        )?;
        if let Some(sp) = &source_path {
            rehead(&tx, project_id, sp)?;
        }
        tx.commit()?;
        drop(conn);
        self.get(id)?.context("promoted document vanished")
    }

    /// The workflow this conversation last named in this project, when the
    /// agent named one: the newest of its own sends (`mcp`) whose workflow
    /// is not the conversation's key. What the hook's next draft joins, so a
    /// conversation is one workflow and not the agent's name plus the hook's
    /// key -- 104 of 114 drafts were in another workflow from their send in
    /// a month (audit bf13a1859b, F7).
    pub fn named_workflow(
        &self,
        project_root: &str,
        session: &str,
    ) -> Result<Option<(String, String)>> {
        if session.trim().is_empty() {
            return Ok(None);
        }
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT w.key, w.title FROM live_docs d
             JOIN projects p ON p.id = d.project_id JOIN workflows w ON w.id = d.workflow_id
             WHERE p.root = ?1 AND d.session = ?2 AND d.origin = 'mcp' AND w.key != LOWER(?2)
             ORDER BY d.received_at DESC, d.rowid DESC LIMIT 1",
            params![project_root, session],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(Into::into)
    }
}
