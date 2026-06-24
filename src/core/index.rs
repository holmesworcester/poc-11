//! Durable storage behind the [`Index`] trait, so SQLite never reaches a
//! projector. [`SqliteIndex`] persists two things (§4): the content-addressed
//! fact log, and the *syntactic* needs/offers index of `Offer<Asserted>`. One
//! reverse key (`edges_by_key`) serves BOTH match directions: need→offer ("needs
//! pull old offerers") and offer→need ("offers pull old needers").
use rusqlite::{params, Connection};

use super::item::{fact_id, FactId};
use super::offer::{EdgeKind, Key, Offer, Role, Scope};
use super::typestate::Asserted;

/// The storage contract core (admit/play) and the daemon's workers depend on. The
/// Stage-1 Verus core is written against this contract, not against rusqlite.
pub trait Index {
    fn insert_asserted(
        &self,
        owner: FactId,
        edges: &[Offer<Asserted>],
        ts: u64,
    ) -> Result<(), String>;
    fn flush_fact(&self, id: FactId, bytes: &[u8], ts: u64) -> Result<(), String>;
    fn load_fact(&self, id: &FactId) -> Result<Option<Vec<u8>>, String>;
    /// need→offer: owners that OFFER `key`.
    fn offers_for_key(&self, role: Role, scope: Scope, key: &Key) -> Result<Vec<FactId>, String>;
    /// offer→need: owners that NEED `key`.
    fn needs_for_key(&self, role: Role, scope: Scope, key: &Key) -> Result<Vec<FactId>, String>;
    /// The bounded replay seed: the newest `n` facts by admission order.
    fn window(&self, n: usize) -> Result<Vec<FactId>, String>;
    fn total_facts(&self) -> Result<usize, String>;
    fn total_edges(&self) -> Result<usize, String>;
}

const SCHEMA: &str = r#"
CREATE TABLE IF NOT EXISTS facts (
    id    BLOB PRIMARY KEY NOT NULL,
    bytes BLOB NOT NULL,
    ts    INTEGER NOT NULL              -- admission order; selects the window (NOT hashed)
);
CREATE INDEX IF NOT EXISTS facts_by_ts ON facts (ts, id);

CREATE TABLE IF NOT EXISTS edges (
    owner    BLOB    NOT NULL,
    kind     INTEGER NOT NULL,          -- 0 = need, 1 = offer
    role     TEXT    NOT NULL,
    scope    TEXT    NOT NULL,
    mkey     BLOB    NOT NULL,          -- match address (= a FactId for links)
    polarity INTEGER NOT NULL,
    binding  INTEGER NOT NULL,
    PRIMARY KEY (owner, kind, role, scope, mkey)
);
-- One reverse key serving both match directions.
CREATE INDEX IF NOT EXISTS edges_by_key ON edges (role, scope, mkey, kind, owner);
"#;

pub struct SqliteIndex {
    conn: Connection,
}

impl SqliteIndex {
    /// Open (or create) a database file and apply the schema. WAL + a busy timeout
    /// let the daemon and CLI processes share the file.
    pub fn open(path: &str) -> Result<Self, String> {
        let conn = Connection::open(path).map_err(stringify)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA busy_timeout=5000;")
            .map_err(stringify)?;
        conn.execute_batch(SCHEMA).map_err(stringify)?;
        Ok(Self { conn })
    }

    /// All stored facts as (id, bytes), oldest first — used by the egress worker.
    pub fn all_facts(&self) -> Result<Vec<(FactId, Vec<u8>)>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT id, bytes FROM facts ORDER BY ts, id")
            .map_err(stringify)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((r.get::<_, Vec<u8>>(0)?, r.get::<_, Vec<u8>>(1)?))
            })
            .map_err(stringify)?;
        let mut out = vec![];
        for row in rows {
            let (id, bytes) = row.map_err(stringify)?;
            out.push((to_id(id)?, bytes));
        }
        Ok(out)
    }

    fn owners_by_key(
        &self,
        role: Role,
        scope: Scope,
        key: &Key,
        kind: i64,
    ) -> Result<Vec<FactId>, String> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT owner FROM edges
                 WHERE role=?1 AND scope=?2 AND mkey=?3 AND kind=?4 ORDER BY owner",
            )
            .map_err(stringify)?;
        let rows = stmt
            .query_map(params![role.0, scope.as_str(), &key.0[..], kind], |r| {
                r.get::<_, Vec<u8>>(0)
            })
            .map_err(stringify)?;
        let mut out = vec![];
        for row in rows {
            out.push(to_id(row.map_err(stringify)?)?);
        }
        Ok(out)
    }
}

fn stringify<E: std::fmt::Display>(e: E) -> String {
    e.to_string()
}

fn to_id(v: Vec<u8>) -> Result<FactId, String> {
    v.try_into()
        .map_err(|_| "stored id is not 32 bytes".to_string())
}

fn kind_code(k: EdgeKind) -> i64 {
    match k {
        EdgeKind::Need => 0,
        EdgeKind::Offer => 1,
    }
}

impl Index for SqliteIndex {
    fn insert_asserted(
        &self,
        owner: FactId,
        edges: &[Offer<Asserted>],
        _ts: u64,
    ) -> Result<(), String> {
        for e in edges {
            self.conn
                .execute(
                    "INSERT OR IGNORE INTO edges
                     (owner, kind, role, scope, mkey, polarity, binding)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    params![
                        &owner[..],
                        kind_code(e.kind),
                        e.role.0,
                        e.scope.as_str(),
                        &e.key.0[..],
                        e.polarity as i64,
                        e.binding as i64,
                    ],
                )
                .map_err(stringify)?;
        }
        Ok(())
    }

    fn flush_fact(&self, id: FactId, bytes: &[u8], ts: u64) -> Result<(), String> {
        // The fact log is content-addressed: never store bytes under a foreign id.
        if fact_id(bytes) != id {
            return Err("flush_fact: id does not match content hash".to_string());
        }
        self.conn
            .execute(
                "INSERT OR IGNORE INTO facts (id, bytes, ts) VALUES (?1, ?2, ?3)",
                params![&id[..], bytes, ts as i64],
            )
            .map_err(stringify)?;
        Ok(())
    }

    fn load_fact(&self, id: &FactId) -> Result<Option<Vec<u8>>, String> {
        self.conn
            .query_row(
                "SELECT bytes FROM facts WHERE id=?1",
                params![&id[..]],
                |r| r.get::<_, Vec<u8>>(0),
            )
            .map(Some)
            .or_else(|e| match e {
                rusqlite::Error::QueryReturnedNoRows => Ok(None),
                other => Err(stringify(other)),
            })
    }

    fn offers_for_key(&self, role: Role, scope: Scope, key: &Key) -> Result<Vec<FactId>, String> {
        self.owners_by_key(role, scope, key, 1)
    }

    fn needs_for_key(&self, role: Role, scope: Scope, key: &Key) -> Result<Vec<FactId>, String> {
        self.owners_by_key(role, scope, key, 0)
    }

    fn window(&self, n: usize) -> Result<Vec<FactId>, String> {
        let mut stmt = self
            .conn
            .prepare("SELECT id FROM facts ORDER BY ts DESC, id DESC LIMIT ?1")
            .map_err(stringify)?;
        let rows = stmt
            .query_map(params![n as i64], |r| r.get::<_, Vec<u8>>(0))
            .map_err(stringify)?;
        let mut out = vec![];
        for row in rows {
            out.push(to_id(row.map_err(stringify)?)?);
        }
        Ok(out)
    }

    fn total_facts(&self) -> Result<usize, String> {
        self.conn
            .query_row("SELECT COUNT(*) FROM facts", [], |r| r.get::<_, i64>(0))
            .map(|n| n as usize)
            .map_err(stringify)
    }

    fn total_edges(&self) -> Result<usize, String> {
        self.conn
            .query_row("SELECT COUNT(*) FROM edges", [], |r| r.get::<_, i64>(0))
            .map(|n| n as usize)
            .map_err(stringify)
    }
}
