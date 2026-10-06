//! Incremental encrypted history. SQLite sees keyed identifiers, timestamps and sizes,
//! never message text or unkeyed search terms. Each payload authenticates its identity.
use crate::{
    search_index::{Filter, IndexedRoom},
    storage::Profile,
};
use matrix_sdk_store_encryption::StoreCipher;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::HashSet,
    sync::{Arc, Mutex},
};

type Result<T> = std::result::Result<T, &'static str>;
const FAILURE: &str = "storage-failed";
#[derive(Clone)]
pub(crate) struct Archive(Arc<Mutex<Store>>);
struct Store {
    persistent: bool,
    db: Connection,
    cipher: StoreCipher,
}
#[derive(Serialize, Deserialize)]
struct Record {
    room: String,
    event: Value,
}
#[derive(Clone, Default, Serialize, Deserialize)]
pub(crate) struct Position {
    pub cursor: Option<String>,
    pub started: bool,
}
#[derive(Clone)]
pub(crate) struct CachedPage {
    pub values: Vec<Value>,
    pub more: bool,
}

impl Default for Archive {
    fn default() -> Self {
        Self::open(None).expect("create temporary history store")
    }
}
impl Archive {
    pub fn open(storage: Option<&(Arc<Profile>, String)>) -> Result<Self> {
        let (db, cipher) = match storage {
            Some((p, id)) => (
                Connection::open(p.store_path(id)?.join("history.sqlite3")).map_err(|_| FAILURE)?,
                p.archive_cipher(id)?,
            ),
            None => (
                Connection::open_in_memory().map_err(|_| FAILURE)?,
                StoreCipher::new().map_err(|_| FAILURE)?,
            ),
        };
        db.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA secure_delete=ON;
            CREATE TABLE IF NOT EXISTS events(room BLOB NOT NULL,id BLOB NOT NULL,ts INTEGER NOT NULL,root INTEGER NOT NULL,pending INTEGER NOT NULL,target BLOB,data BLOB NOT NULL,PRIMARY KEY(room,id));
            CREATE INDEX IF NOT EXISTS chronology ON events(room,root,ts DESC,id);
            CREATE INDEX IF NOT EXISTS relations ON events(room,target);
            CREATE TABLE IF NOT EXISTS terms(room BLOB NOT NULL,id BLOB NOT NULL,term BLOB NOT NULL,PRIMARY KEY(room,term,id));
            CREATE INDEX IF NOT EXISTS event_terms ON terms(room,id);
            CREATE TABLE IF NOT EXISTS tombstones(room BLOB NOT NULL,id BLOB NOT NULL,PRIMARY KEY(room,id));
            CREATE TABLE IF NOT EXISTS jobs(room BLOB NOT NULL,job BLOB NOT NULL,data BLOB NOT NULL,PRIMARY KEY(room,job));
            CREATE TABLE IF NOT EXISTS cursors(room BLOB NOT NULL,job BLOB NOT NULL,cursor BLOB NOT NULL,PRIMARY KEY(room,job,cursor));
            CREATE TABLE IF NOT EXISTS meta(name TEXT PRIMARY KEY,value INTEGER NOT NULL);").map_err(|_| FAILURE)?;
        let result = Self(Arc::new(Mutex::new(Store {
            db,
            cipher,
            persistent: storage.is_some(),
        })));
        if let Some((p, id)) = storage {
            result.migrate(p, id)?;
        }
        Ok(result)
    }
    pub fn counts(&self) -> Result<(u64, u64)> {
        self.with(|s| {
            s.db.query_row(
                "SELECT COUNT(*), COALESCE(SUM(pending),0) FROM events",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .map_err(|_| FAILURE)
        })
    }
    pub fn persistent(&self) -> bool {
        self.with(|s| Ok(s.persistent)).unwrap_or(false)
    }
    fn with<T>(&self, f: impl FnOnce(&mut Store) -> Result<T>) -> Result<T> {
        f(&mut *self.0.lock().map_err(|_| FAILURE)?)
    }
    fn migrate(&self, profile: &Profile, id: &str) -> Result<()> {
        let migrated = self.with(|s| {
            Ok(s.db
                .query_row(
                    "SELECT value FROM meta WHERE name='legacy-import'",
                    [],
                    |r| r.get::<_, i64>(0),
                )
                .optional()
                .map_err(|_| FAILURE)?
                .is_some())
        })?;
        if migrated {
            return Ok(());
        }
        let legacy: crate::search_index::Index = profile.read_data(id, "search-index")?;
        for (room, index) in legacy.rooms {
            // Persist tombstones first; a crash and retry cannot resurrect earlier versions.
            self.with(|s| {
                let room = s.hash("room", &room);
                for event in &index.redacted {
                    s.db.execute(
                        "INSERT OR IGNORE INTO tombstones VALUES(?1,?2)",
                        params![room, s.hash("event", event)],
                    )
                    .map_err(|_| FAILURE)?;
                }
                Ok(())
            })?;
            let mut values = index.events.into_values();
            loop {
                let page: Vec<_> = values.by_ref().take(50).collect();
                if page.is_empty() {
                    break;
                }
                self.merge(&room, page, true)?;
            }
        }
        self.with(|s| {
            s.db.execute("INSERT INTO meta VALUES('legacy-import',1)", [])
                .map_err(|_| FAILURE)?;
            Ok(())
        })
    }
    pub fn merge(&self, room: &str, values: Vec<Value>, redacts: bool) -> Result<bool> {
        self.with(|s| s.transaction(|s| s.merge(room, values, redacts)))
    }
    pub fn position(&self, room: &str, job: &str) -> Result<Position> {
        self.with(|s| s.position(room, job))
    }
    pub fn gap(&self, room: &str, cursor: &str) -> Result<()> {
        self.with(|s| {
            let key = format!("gap:{cursor}");
            let data = s
                .cipher
                .encrypt_value(&(
                    room,
                    &key,
                    Position {
                        cursor: Some(cursor.to_owned()),
                        started: false,
                    },
                ))
                .map_err(|_| FAILURE)?;
            s.db.execute(
                "INSERT OR IGNORE INTO jobs VALUES(?1,?2,?3)",
                params![s.hash("room", room), s.hash("job", &key), data],
            )
            .map_err(|_| FAILURE)?;
            Ok(())
        })
    }
    pub fn jobs(&self, room: &str) -> Result<Vec<String>> {
        self.with(|s| {
            let mut stmt =
                s.db.prepare("SELECT data FROM jobs WHERE room=?1")
                    .map_err(|_| FAILURE)?;
            let rows = stmt
                .query_map([s.hash("room", room)], |r| r.get::<_, Vec<u8>>(0))
                .map_err(|_| FAILURE)?;
            let mut jobs = vec!["history".to_owned()];
            for row in rows {
                let (r, j, p): (String, String, Position) = s
                    .cipher
                    .decrypt_value(&row.map_err(|_| FAILURE)?)
                    .map_err(|_| FAILURE)?;
                if r != room {
                    return Err(FAILURE);
                }
                if j != "history" && (!p.started || p.cursor.is_some()) {
                    jobs.push(j);
                }
            }
            Ok(jobs)
        })
    }
    /// Compare-and-commit prevents a search request and background fetch advancing the same cursor twice.
    pub fn commit_page(
        &self,
        room: &str,
        job: &str,
        from: &Position,
        next: Option<String>,
        values: Vec<Value>,
        redacts: bool,
    ) -> Result<bool> {
        self.with(|s| {
            s.transaction(|s| {
                let current = s.position(room, job)?;
                if current.cursor != from.cursor || current.started != from.started {
                    return Ok(false);
                }
                let rh = s.hash("room", room);
                let jh = s.hash("job", job);
                if let Some(next) = &next {
                    if from.cursor.as_ref() == Some(next) {
                        return Err("history-stalled");
                    }
                    let inserted =
                        s.db.execute(
                            "INSERT OR IGNORE INTO cursors VALUES(?1,?2,?3)",
                            params![rh, jh, s.hash("cursor", next)],
                        )
                        .map_err(|_| FAILURE)?;
                    if inserted == 0 {
                        return Err("history-stalled");
                    }
                }
                let overlap = if job != "history" {
                    values.iter().any(|v| {
                        v["event_id"]
                            .as_str()
                            .is_some_and(|id| s.exists(room, id).unwrap_or(false))
                    })
                } else {
                    false
                };
                s.merge(room, values, redacts)?;
                let state = Position {
                    cursor: if overlap { None } else { next },
                    started: true,
                };
                let data = s
                    .cipher
                    .encrypt_value(&(room, job, state))
                    .map_err(|_| FAILURE)?;
                s.db.execute(
                    "INSERT OR REPLACE INTO jobs VALUES(?1,?2,?3)",
                    params![rh, jh, data],
                )
                .map_err(|_| FAILURE)?;
                Ok(true)
            })
        })
    }
    pub fn recent(&self, room: &str, limit: usize, redacts: bool) -> Result<CachedPage> {
        self.with(|s| {
            let rh=s.hash("room",room);
            let mut stmt=s.db.prepare("SELECT id FROM events WHERE room=?1 AND root=1 ORDER BY ts DESC,id DESC LIMIT ?2").map_err(|_| FAILURE)?;
            let ids=stmt.query_map(params![rh,(limit+1) as i64], |r| r.get::<_,Vec<u8>>(0)).map_err(|_| FAILURE)?.collect::<std::result::Result<Vec<_>,_>>().map_err(|_| FAILURE)?;
            let more=ids.len()>limit || !s.position(room,"history")?.started || s.position(room,"history")?.cursor.is_some();
            let mut values=Vec::new();
            for id in ids.iter().take(limit).rev() { values.extend(s.related(room,id)?.events.into_values()); }
            values.sort_by_key(|v| v["origin_server_ts"].as_u64().unwrap_or(0));
            // Projection is done by History so all existing relation rules remain shared.
            let _=redacts;
            Ok(CachedPage {values,more})
        })
    }
    pub fn query(&self, room: &str, query: &str, redacts: bool) -> Result<crate::SearchPage> {
        let filter = Filter::parse(query)?;
        self.with(|s| {
            let rh=s.hash("room",room);
            let grams=grams(&filter.text);
            // A keyed trigram narrows candidates. Final projection verifies the complete phrase,
            // edits and filters; collisions and old edit terms cannot produce false hits.
            let sql=if grams.is_empty() {
                "SELECT id FROM events WHERE room=?1 AND root=1 ORDER BY ts DESC,id DESC"
            } else {
                "SELECT DISTINCT COALESCE(e.target,e.id) FROM events e JOIN terms t ON e.room=t.room AND e.id=t.id WHERE e.room=?1 AND t.term=?2 ORDER BY e.ts DESC"
            };
            let mut stmt=s.db.prepare(sql).map_err(|_| FAILURE)?;
            let hash=grams.first().map(|g| s.hash("term",g));
            let mut rows=if let Some(hash)=hash {stmt.query(params![rh,hash])} else {stmt.query(params![rh])}.map_err(|_| FAILURE)?;
            let mut messages=Vec::new();
            while let Some(row)=rows.next().map_err(|_| FAILURE)? {
                let id: Vec<u8>=row.get(0).map_err(|_| FAILURE)?;
                messages.extend(crate::search_index::project(&s.related(room,&id)?, &filter, redacts).messages);
                if messages.len()>1000 {break;}
            }
            messages.sort_by_key(|m| std::cmp::Reverse(m.timestamp));
            let truncated=messages.len()>1000; messages.truncate(1000);
            let (scanned,unavailable): (usize,usize)=s.db.query_row("SELECT COUNT(*),COALESCE(SUM(pending),0) FROM events WHERE room=?1",[rh], |r| Ok((r.get(0)?,r.get(1)?))).map_err(|_| FAILURE)?;
            let p=s.position(room,"history")?;
            Ok(crate::SearchPage {messages,scanned,unavailable,more:!p.started || p.cursor.is_some(),truncated})
        })
    }
    pub async fn search(
        &self,
        room: &matrix_sdk::Room,
        query: &str,
        more: bool,
    ) -> Result<crate::SearchPage> {
        Filter::parse(query)?;
        let id = room.room_id().as_str();
        let redacts = room
            .clone_info()
            .room_version_rules_or_default()
            .redaction
            .keep_room_redaction_redacts;
        let page = self.query(id, query, redacts)?;
        if more || page.scanned == 0 {
            crate::archive_sync::fetch(self, room, "history").await?;
        }
        let archive = self.clone();
        let id = id.to_owned();
        let query = query.to_owned();
        tokio::task::spawn_blocking(move || archive.query(&id, &query, redacts))
            .await
            .map_err(|_| FAILURE)?
    }
    pub fn pending(&self, room: &str, after: &[u8]) -> Result<Vec<(Vec<u8>, Value)>> {
        self.with(|s| {
            let mut stmt=s.db.prepare("SELECT id,data FROM events WHERE room=?1 AND pending=1 AND id>?2 ORDER BY id LIMIT 50").map_err(|_| FAILURE)?;
            let rows=stmt.query_map(params![s.hash("room",room),after], |r| Ok((r.get::<_,Vec<u8>>(0)?,r.get::<_,Vec<u8>>(1)?))).map_err(|_| FAILURE)?;
            rows.map(|row| {let(id,data)=row.map_err(|_| FAILURE)?; Ok((id.clone(),s.decode(room,&id,&data)?))}).collect()
        })
    }
}
fn grams(text: &str) -> Vec<String> {
    let chars: Vec<_> = text.to_lowercase().chars().collect();
    chars
        .windows(3)
        .map(|s| s.iter().collect())
        .collect::<HashSet<String>>()
        .into_iter()
        .collect()
}
impl Store {
    fn hash(&self, table: &str, value: &str) -> Vec<u8> {
        self.cipher.hash_key(table, value.as_bytes()).to_vec()
    }
    fn transaction<T>(&mut self, f: impl FnOnce(&mut Self) -> Result<T>) -> Result<T> {
        self.db
            .execute_batch("BEGIN IMMEDIATE")
            .map_err(|_| FAILURE)?;
        match f(self) {
            Ok(value) => {
                if self.db.execute_batch("COMMIT").is_err() {
                    let _ = self.db.execute_batch("ROLLBACK");
                    return Err(FAILURE);
                }
                Ok(value)
            }
            Err(e) => {
                let _ = self.db.execute_batch("ROLLBACK");
                Err(e)
            }
        }
    }
    fn decode(&self, room: &str, id: &[u8], data: &[u8]) -> Result<Value> {
        let r: Record = self.cipher.decrypt_value(data).map_err(|_| FAILURE)?;
        if r.room != room
            || r.event["event_id"]
                .as_str()
                .is_none_or(|v| self.hash("event", v) != id)
        {
            return Err(FAILURE);
        }
        Ok(r.event)
    }
    fn exists(&self, room: &str, id: &str) -> Result<bool> {
        self.db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM events WHERE room=?1 AND id=?2)",
                params![self.hash("room", room), self.hash("event", id)],
                |r| r.get(0),
            )
            .map_err(|_| FAILURE)
    }
    fn position(&self, room: &str, job: &str) -> Result<Position> {
        let data: Option<Vec<u8>> = self
            .db
            .query_row(
                "SELECT data FROM jobs WHERE room=?1 AND job=?2",
                params![self.hash("room", room), self.hash("job", job)],
                |r| r.get(0),
            )
            .optional()
            .map_err(|_| FAILURE)?;
        match data {
            None => Ok(Position::default()),
            Some(data) => {
                let (r, j, p): (String, String, Position) =
                    self.cipher.decrypt_value(&data).map_err(|_| FAILURE)?;
                if r != room || j != job {
                    return Err(FAILURE);
                }
                Ok(p)
            }
        }
    }
    fn related(&self, room: &str, id: &[u8]) -> Result<IndexedRoom> {
        let rh = self.hash("room", room);
        let mut stmt=self.db.prepare("SELECT id,data FROM events WHERE room=?1 AND (id=?2 OR target=?2 OR target IN (SELECT id FROM events WHERE room=?1 AND target=?2))").map_err(|_| FAILURE)?;
        let rows = stmt
            .query_map(params![rh, id], |r| {
                Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, Vec<u8>>(1)?))
            })
            .map_err(|_| FAILURE)?;
        let mut indexed = IndexedRoom::default();
        for row in rows {
            let (id, data) = row.map_err(|_| FAILURE)?;
            let v = self.decode(room, &id, &data)?;
            indexed
                .events
                .insert(v["event_id"].as_str().ok_or(FAILURE)?.into(), v);
        }
        Ok(indexed)
    }
    fn merge(&mut self, room: &str, values: Vec<Value>, redacts: bool) -> Result<bool> {
        let rh = self.hash("room", room);
        let mut erased = HashSet::new();
        for v in &values {
            if v["unsigned"].get("redacted_because").is_some()
                && let Some(id) = v["event_id"].as_str()
            {
                erased.insert(id.to_owned());
            }
            if v["type"] == "m.room.redaction"
                && let Some(id) = (if redacts {
                    &v["content"]["redacts"]
                } else {
                    &v["redacts"]
                })
                .as_str()
            {
                erased.insert(id.to_owned());
            }
        }
        for id in &erased {
            self.db
                .execute(
                    "INSERT OR IGNORE INTO tombstones VALUES(?1,?2)",
                    params![rh, self.hash("event", id)],
                )
                .map_err(|_| FAILURE)?;
        }
        for value in values {
            self.put(room, value, redacts)?;
        }
        for id in erased {
            let related = self.related(room, &self.hash("event", &id))?;
            for v in related.events.into_values() {
                self.put(room, v, redacts)?;
            }
        }
        Ok(true)
    }
    fn put(&self, room: &str, mut value: Value, redacts: bool) -> Result<()> {
        let Some(id) = value["event_id"].as_str().map(str::to_owned) else {
            return Ok(());
        };
        let rh = self.hash("room", room);
        let ih = self.hash("event", &id);
        let rel = &value["content"]["m.relates_to"];
        let target = if value["type"] == "m.room.redaction" {
            if redacts {
                value["content"]["redacts"].as_str()
            } else {
                value["redacts"].as_str()
            }
        } else if rel["rel_type"] == "m.replace" || rel["rel_type"] == "m.annotation" {
            rel["event_id"].as_str()
        } else {
            None
        };
        let target = target.map(|id| self.hash("event", id));
        let tombstone: bool = self
            .db
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM tombstones WHERE room=?1 AND (id=?2 OR id=?3))",
                params![
                    rh,
                    ih,
                    if rel["rel_type"] == "m.replace" {
                        target.as_ref()
                    } else {
                        None
                    }
                ],
                |r| r.get(0),
            )
            .map_err(|_| FAILURE)?;
        if tombstone {
            value["content"] = serde_json::json!({});
            value["unsigned"] = serde_json::json!({"redacted_because":{}});
        }
        let pending = value["type"] == "m.room.encrypted";
        if pending && !tombstone {
            let decrypted: bool = self
                .db
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM events WHERE room=?1 AND id=?2 AND pending=0)",
                    params![rh, ih],
                    |r| r.get(0),
                )
                .map_err(|_| FAILURE)?;
            if decrypted {
                return Ok(());
            }
        }
        let root = target.is_none()
            && crate::timeline::decode(value.clone(), redacts)
                .message
                .is_some();
        let ts = value["origin_server_ts"]
            .as_u64()
            .unwrap_or(0)
            .min(i64::MAX as u64) as i64;
        let data = self
            .cipher
            .encrypt_value(&Record {
                room: room.into(),
                event: value.clone(),
            })
            .map_err(|_| FAILURE)?;
        self.db
            .execute(
                "INSERT OR REPLACE INTO events VALUES(?1,?2,?3,?4,?5,?6,?7)",
                params![rh, ih, ts, root, pending, target, data],
            )
            .map_err(|_| FAILURE)?;
        self.db
            .execute("DELETE FROM terms WHERE room=?1 AND id=?2", params![rh, ih])
            .map_err(|_| FAILURE)?;
        for body in [
            value["content"]["body"].as_str(),
            value["content"]["m.new_content"]["body"].as_str(),
        ]
        .into_iter()
        .flatten()
        {
            for term in grams(body) {
                self.db
                    .execute(
                        "INSERT OR IGNORE INTO terms VALUES(?1,?2,?3)",
                        params![rh, ih, self.hash("term", &term)],
                    )
                    .map_err(|_| FAILURE)?;
            }
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn event(i: u64, body: &str) -> Value {
        json!({"type":"m.room.message","event_id":format!("$event{i}"),"sender":"@alice:local","origin_server_ts":i,"content":{"msgtype":"m.text","body":body}})
    }
    #[test]
    fn archive_pages_are_bounded_and_search_reaches_older_history() {
        let archive = Archive::default();
        for page in 0..12 {
            archive
                .merge(
                    "!room:local",
                    (page * 50..page * 50 + 50)
                        .map(|i| event(i, &format!("message {i}")))
                        .collect(),
                    false,
                )
                .unwrap();
        }
        let page = archive.recent("!room:local", 50, false).unwrap();
        assert_eq!(page.values.len(), 50);
        assert_eq!(page.values[0]["origin_server_ts"], 550);
        let result = archive
            .query(
                "!room:local",
                "message 12 from:@alice:local after:12 before:13",
                false,
            )
            .unwrap();
        assert_eq!(result.messages.len(), 1);
        assert_eq!(result.messages[0].id, "$event12");
        assert_eq!(
            archive
                .recent("!room:local", 100, false)
                .unwrap()
                .values
                .len(),
            100
        );
    }
    #[test]
    fn cursor_commit_is_atomic_resumable_and_rejects_cycles() {
        let a = Archive::default();
        let from = a.position("!room:local", "history").unwrap();
        assert!(
            a.commit_page(
                "!room:local",
                "history",
                &from,
                Some("page2".into()),
                vec![event(1, "one")],
                false
            )
            .unwrap()
        );
        assert!(
            !a.commit_page(
                "!room:local",
                "history",
                &from,
                None,
                vec![event(2, "stale")],
                false
            )
            .unwrap()
        );
        let from = a.position("!room:local", "history").unwrap();
        assert!(
            a.commit_page(
                "!room:local",
                "history",
                &from,
                Some("page2".into()),
                vec![event(3, "rollback")],
                false
            )
            .is_err()
        );
        assert!(
            a.query("!room:local", "rollback", false)
                .unwrap()
                .messages
                .is_empty()
        );
        assert_eq!(
            a.position("!room:local", "history").unwrap().cursor,
            Some("page2".into())
        );
        a.commit_page("!room:local", "history", &from, None, vec![], false)
            .unwrap();
        assert!(!a.query("!room:local", "one", false).unwrap().more);
    }
    #[test]
    fn edits_and_redactions_do_not_leak_stale_search_hits() {
        let a = Archive::default();
        a.merge("!room:local", vec![event(1, "original secret")], false)
            .unwrap();
        let edit = json!({"type":"m.room.message","event_id":"$edit","sender":"@alice:local","origin_server_ts":2,"content":{"msgtype":"m.text","body":"* changed","m.relates_to":{"rel_type":"m.replace","event_id":"$event1"},"m.new_content":{"msgtype":"m.text","body":"changed"}}});
        a.merge("!room:local", vec![edit.clone()], false).unwrap();
        assert_eq!(
            a.query("!room:local", "changed", false)
                .unwrap()
                .messages
                .len(),
            1
        );
        assert!(
            a.query("!room:local", "secret", false)
                .unwrap()
                .messages
                .is_empty()
        );
        a.merge("!room:local",vec![json!({"type":"m.room.redaction","event_id":"$redact","redacts":"$event1","sender":"@alice:local","content":{}})],false).unwrap();
        a.merge(
            "!room:local",
            vec![event(1, "original secret"), edit],
            false,
        )
        .unwrap();
        assert!(
            a.query("!room:local", "changed", false)
                .unwrap()
                .messages
                .is_empty()
        );
        assert!(
            a.query("!room:local", "secret", false)
                .unwrap()
                .messages
                .is_empty()
        );
    }
    #[test]
    fn encrypted_rows_authenticate_identity_and_plaintext_is_not_stored() {
        let a = Archive::default();
        a.merge("!room:local", vec![event(1, "secret sentinel")], false)
            .unwrap();
        a.with(|s| {
            let data: Vec<u8> =
                s.db.query_row("SELECT data FROM events", [], |r| r.get(0))
                    .unwrap();
            assert!(!data.windows(6).any(|b| b == b"secret"));
            assert!(
                s.decode("!another:local", &s.hash("event", "$event1"), &data)
                    .is_err()
            );
            assert!(
                s.decode("!room:local", &s.hash("event", "$event2"), &data)
                    .is_err()
            );
            Ok(())
        })
        .unwrap();
    }
}
