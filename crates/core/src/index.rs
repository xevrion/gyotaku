use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::Result;
use rusqlite::{
    Connection, OptionalExtension, TransactionBehavior, params, params_from_iter, types::Value,
};

use crate::burst::{Likeness, Shape, WINDOW, alike};
use crate::query::{Filter, Query};
use crate::{Line, Rect, Shot};

// Bump this whenever the tables change. The index is a cache of OCR output,
// so an unknown version gets dropped and rebuilt, but reading everything
// again takes most of an hour on a big folder, so a version that can be
// brought forward from the text already stored is, see `init`.
const SCHEMA: i32 = 3;

// The trigram tokenizer indexes every 3 character window, which is what makes
// "nutsmp" find "donutsmp.net". The flip side is that it cannot match anything
// shorter than 3 characters, see `search`.
const TABLES: &str = "
    CREATE TABLE shots (
        id     INTEGER PRIMARY KEY,
        path   TEXT NOT NULL UNIQUE,
        mtime  INTEGER NOT NULL,
        width  INTEGER NOT NULL,
        height INTEGER NOT NULL
    );
    CREATE TABLE lines (
        id      INTEGER PRIMARY KEY,
        shot_id INTEGER NOT NULL REFERENCES shots(id) ON DELETE CASCADE,
        text    TEXT NOT NULL,
        x REAL NOT NULL, y REAL NOT NULL, w REAL NOT NULL, h REAL NOT NULL,
        score   REAL NOT NULL
    );
    CREATE INDEX lines_by_shot ON lines(shot_id);
    CREATE VIRTUAL TABLE shots_fts USING fts5(text, tokenize = 'trigram');
";

// The same text again, folded (see `fold`), for the near matches. Added in
// version 2. The trigger is plain SQL so it also runs when an older reader
// still has the index open and deletes a shot.
const NEAR: &str = "
    CREATE VIRTUAL TABLE shots_near USING fts5(text, tokenize = 'trigram');
    CREATE TRIGGER shots_near_gone AFTER DELETE ON shots BEGIN
        DELETE FROM shots_near WHERE rowid = old.id;
    END;
";

// Bursts of near-identical shots, version 3, see `burst`. `look` is the
// thumbnail's difference hash and `burst` names the burst a shot is in, by
// the lowest id in it. Both start out empty for shots read before version
// 3, and a burst that's NULL is one the reader still has to work out.
const BURSTS: &str = "
    ALTER TABLE shots ADD COLUMN look INTEGER;
    ALTER TABLE shots ADD COLUMN burst INTEGER;
    CREATE INDEX shots_by_mtime ON shots(mtime);
";

pub struct Index {
    db: Connection,
}

#[derive(Debug, Clone)]
pub struct Hit {
    pub id: i64,
    pub path: PathBuf,
    pub mtime: i64,
    pub width: u32,
    pub height: u32,
    /// Only the lines that contain a query term, so the UI can draw a box
    /// around each one. Empty when browsing without a query.
    pub lines: Vec<Line>,
    /// Found only once look-alike characters were treated as one, like `0`
    /// for `O`. These always come after every exact match.
    pub near: bool,
    pub look: Option<u64>,
    /// The burst of near-identical shots this one is in, named by the
    /// lowest id in it, or its own id when it's alone (see `burst`).
    pub burst: i64,
}

impl Index {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        Self::init(Connection::open(path)?)
    }

    pub fn open_default() -> Result<Self> {
        Self::open(&crate::data_dir()?.join("index.db"))
    }

    pub fn open_in_memory() -> Result<Self> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(db: Connection) -> Result<Self> {
        // WAL so the watcher can write while the app is reading.
        db.pragma_update(None, "journal_mode", "WAL")?;
        db.pragma_update(None, "synchronous", "NORMAL")?;
        db.pragma_update(None, "foreign_keys", true)?;

        // The app and the watcher can both open a brand new index at the same
        // moment. Checking and creating the tables under one write lock means
        // one of them does it and the other finds it done.
        let mut db = db;
        let tx = db.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let version: i32 = tx.pragma_query_value(None, "user_version", |r| r.get(0))?;
        match version {
            SCHEMA => {}
            // Version 1 had everything but the folded text, which can be
            // made from the text it already has, and version 2 everything
            // but the bursts, which the reader works out in the background.
            1 | 2 => {
                if version == 1 {
                    tx.execute_batch(NEAR)?;
                    let mut all = tx.prepare("SELECT rowid, text FROM shots_fts")?;
                    let mut add =
                        tx.prepare("INSERT INTO shots_near (rowid, text) VALUES (?1, ?2)")?;
                    let mut rows = all.query([])?;
                    while let Some(r) = rows.next()? {
                        let text: String = r.get(1)?;
                        add.execute(params![r.get::<_, i64>(0)?, fold(&text)])?;
                    }
                }
                tx.execute_batch(BURSTS)?;
                tx.pragma_update(None, "user_version", SCHEMA)?;
            }
            _ => {
                tx.execute_batch(
                    "DROP TABLE IF EXISTS shots_near;
                     DROP TABLE IF EXISTS shots_fts;
                     DROP TABLE IF EXISTS lines;
                     DROP TABLE IF EXISTS shots;",
                )?;
                tx.execute_batch(TABLES)?;
                tx.execute_batch(NEAR)?;
                tx.execute_batch(BURSTS)?;
                tx.pragma_update(None, "user_version", SCHEMA)?;
            }
        }
        tx.commit()?;
        Ok(Self { db })
    }

    /// Every change reads first (is that path already there?) and then
    /// writes. A plain transaction would only ask for the write lock halfway
    /// through, and if the other process (the app or the watcher) wrote in
    /// between, SQLite gives up at once instead of waiting. Taking the lock
    /// up front waits out the busy timeout like any other write.
    fn write(&mut self) -> Result<rusqlite::Transaction<'_>> {
        Ok(self
            .db
            .transaction_with_behavior(TransactionBehavior::Immediate)?)
    }

    /// True if this exact file (same path, same mtime) is already indexed.
    pub fn is_current(&self, path: &Path, mtime: i64) -> Result<bool> {
        let found = self
            .db
            .query_row(
                "SELECT 1 FROM shots WHERE path = ?1 AND mtime = ?2",
                params![path_str(path), mtime],
                |_| Ok(()),
            )
            .optional()?;
        Ok(found.is_some())
    }

    /// Whether this path is indexed as a real, showable screenshot.
    pub fn is_visible(&self, path: &Path) -> Result<bool> {
        let found = self
            .db
            .query_row(
                "SELECT 1 FROM shots WHERE path = ?1 AND width > 0",
                [path_str(path)],
                |_| Ok(()),
            )
            .optional()?;
        Ok(found.is_some())
    }

    /// Stores a shot and its lines, replacing whatever was there for the same path.
    pub fn insert(&mut self, shot: &Shot, lines: &[Line]) -> Result<i64> {
        let tx = self.write()?;
        let path = path_str(&shot.path);
        delete(&tx, &path)?;

        tx.execute(
            "INSERT INTO shots (path, mtime, width, height, look) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                path,
                shot.mtime,
                shot.width,
                shot.height,
                shot.look.map(|l| l as i64)
            ],
        )?;
        let id = tx.last_insert_rowid();

        {
            let mut add = tx.prepare(
                "INSERT INTO lines (shot_id, text, x, y, w, h, score)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            )?;
            for l in lines {
                let r = l.rect;
                add.execute(params![id, l.text, r.x, r.y, r.w, r.h, l.score])?;
            }
        }

        // One fts row per screenshot, not per line, so a query like
        // "invoice march" matches even when the two words sit on different lines.
        let text = lines
            .iter()
            .map(|l| l.text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        tx.execute(
            "INSERT INTO shots_fts (rowid, text) VALUES (?1, ?2)",
            params![id, text],
        )?;
        tx.execute(
            "INSERT INTO shots_near (rowid, text) VALUES (?1, ?2)",
            params![id, fold(&text)],
        )?;
        settle(&tx, id)?;

        tx.commit()?;
        Ok(id)
    }

    /// Shots whose burst hasn't been worked out yet, newest first, with
    /// whether their look is known. Those are the ones read before bursts
    /// existed; the reader settles them in the background, see `settle`.
    pub fn unsettled(&self, limit: usize) -> Result<Vec<(i64, PathBuf, bool)>> {
        let mut stmt = self.db.prepare_cached(
            "SELECT id, path, look IS NOT NULL FROM shots
             WHERE burst IS NULL AND width > 0 ORDER BY mtime DESC LIMIT ?1",
        )?;
        let rows = stmt
            .query_map([limit as i64], |r| {
                Ok((r.get(0)?, PathBuf::from(r.get::<_, String>(1)?), r.get(2)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(rows)
    }

    /// Records a shot's look when there is one, and puts it in its burst.
    pub fn settle(&mut self, id: i64, look: Option<u64>) -> Result<()> {
        let tx = self.write()?;
        if let Some(look) = look {
            tx.execute(
                "UPDATE shots SET look = ?2 WHERE id = ?1",
                params![id, look as i64],
            )?;
        }
        settle(&tx, id)?;
        tx.commit()?;
        Ok(())
    }

    /// Moves a shot to a new path without touching its text, since a rename
    /// doesn't change a single pixel. Returns false if `from` wasn't indexed.
    pub fn rename(&mut self, from: &Path, to: &Path) -> Result<bool> {
        let tx = self.write()?;
        delete(&tx, &path_str(to))?;
        let moved = tx.execute(
            "UPDATE shots SET path = ?2 WHERE path = ?1",
            params![path_str(from), path_str(to)],
        )?;
        tx.commit()?;
        Ok(moved > 0)
    }

    /// Returns whether there was anything to remove.
    pub fn remove(&mut self, path: &Path) -> Result<bool> {
        let tx = self.write()?;
        let removed = delete(&tx, &path_str(path))?;
        tx.commit()?;
        Ok(removed)
    }

    /// Drops every shot whose file is gone, returns how many.
    pub fn prune(&mut self) -> Result<usize> {
        let gone: Vec<PathBuf> = self.paths()?.into_iter().filter(|p| !p.exists()).collect();
        for p in &gone {
            self.remove(p)?;
        }
        Ok(gone.len())
    }

    pub fn paths(&self) -> Result<Vec<PathBuf>> {
        let mut stmt = self.db.prepare("SELECT path FROM shots")?;
        let paths = stmt
            .query_map([], |r| r.get::<_, String>(0))?
            .map(|p| p.map(PathBuf::from))
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(paths)
    }

    pub fn len(&self) -> Result<usize> {
        let n: i64 = self
            .db
            .query_row("SELECT count(*) FROM shots", [], |r| r.get(0))?;
        Ok(n as usize)
    }

    /// Everything in `shots` has been looked at, but only these can be shown.
    pub fn visible_len(&self) -> Result<usize> {
        let n: i64 = self
            .db
            .query_row("SELECT count(*) FROM shots WHERE width > 0", [], |r| {
                r.get(0)
            })?;
        Ok(n as usize)
    }

    pub fn is_empty(&self) -> Result<bool> {
        Ok(self.len()? == 0)
    }

    /// Every whitespace separated term has to appear somewhere in the shot.
    /// An empty query returns the newest shots, which is what the app shows
    /// before you type anything. Each hit comes with the lines that matched.
    pub fn search(&self, query: &str, limit: usize) -> Result<Vec<Hit>> {
        let mut hits = self.find(query, limit)?;
        for hit in &mut hits {
            hit.lines = self.matching_lines(hit.id, query, hit.near)?;
        }
        Ok(hits)
    }

    /// Same as `search` but without the lines. Fetching those is most of the
    /// cost of a broad query (two letters can match 2000 shots), and a grid
    /// only ever shows a few dozen at a time, so the app asks per tile.
    ///
    /// Exact matches come first. If they don't fill `limit`, the near
    /// matches follow: shots that only match once look-alike characters
    /// are folded together, so `0RDER` and `0rcler` turn up for "order".
    ///
    /// Filters (`in:`, `date:`, `before:`, `after:`, see `Query`) narrow
    /// both. A query of only filters lists what they let through, newest
    /// first.
    pub fn find(&self, query: &str, limit: usize) -> Result<Vec<Hit>> {
        let query = Query::parse(query);
        let filters = &query.filters;
        let (long, short): (Vec<&str>, Vec<&str>) = query
            .terms
            .iter()
            .map(String::as_str)
            .partition(|t| t.chars().count() >= 3);

        let mut hits = self.find_in(false, &long, &short, filters, limit)?;
        // Fewer than `limit` means that's every exact match there is, so
        // anything new below is near. A query with no word long enough to
        // be misread has no near matches.
        let terms: Vec<Vec<char>> = long.iter().map(|t| lower(t)).collect();
        if terms.iter().all(|t| swaps(t) == 0) || hits.len() >= limit {
            return Ok(hits);
        }
        let exact: HashSet<i64> = hits.iter().map(|h| h.id).collect();
        let folded: Vec<String> = long.iter().map(|t| fold(t)).collect();
        let folded: Vec<&str> = folded.iter().map(String::as_str).collect();
        let mut text = self
            .db
            .prepare_cached("SELECT text FROM shots_fts WHERE rowid = ?1")?;
        // The fold only narrows it down. Folded, "internally" holds "email"
        // (rn for m, l for i), so each one is checked against the text as it
        // was read for how many letters had to be taken for others.
        for hit in self.find_in(true, &folded, &short, filters, limit)? {
            if hits.len() >= limit {
                break;
            }
            if exact.contains(&hit.id) {
                continue;
            }
            let read: String = text.query_row([hit.id], |r| r.get(0))?;
            let read = lower(&read);
            if terms.iter().all(|t| reads_near(&read, t, swaps(t))) {
                hits.push(Hit { near: true, ..hit });
            }
        }
        Ok(hits)
    }

    fn find_in(
        &self,
        near: bool,
        long: &[&str],
        short: &[&str],
        filters: &[Filter],
        limit: usize,
    ) -> Result<Vec<Hit>> {
        let (table, alias) = if near {
            ("shots_near", "n")
        } else {
            ("shots_fts", "f")
        };
        let mut sql = format!(
            "SELECT s.id, s.path, s.mtime, s.width, s.height, s.look,
                    COALESCE(s.burst, s.id)
             FROM {table} {alias} JOIN shots s ON s.id = {alias}.rowid"
        );
        // Short terms are matched against the text as it was read, even
        // for near matches.
        if near && !short.is_empty() {
            sql.push_str(" JOIN shots_fts f ON f.rowid = s.id");
        }
        sql.push_str(" WHERE s.width > 0");
        let mut args: Vec<Value> = Vec::new();

        if !long.is_empty() {
            sql.push_str(&format!(" AND {table} MATCH ?"));
            args.push(Value::Text(match_expr(long)));
        }
        // Terms under 3 characters are invisible to the trigram index, so they
        // fall back to LIKE. That is a scan, but only over rows the MATCH above
        // already narrowed down, or over everything for a query like "ip".
        for t in short {
            sql.push_str(" AND f.text LIKE ? ESCAPE '\\'");
            args.push(Value::Text(format!("%{}%", escape_like(t))));
        }
        for f in filters {
            match f {
                // A folder starting with the name, followed by a separator
                // somewhere later, so it's a folder on the way to the file
                // and not the file's own name. LIKE ignores case (ASCII only).
                Filter::In(name) => {
                    let name = escape_like(name);
                    sql.push_str(" AND (s.path LIKE ? ESCAPE '\\' OR s.path LIKE ? ESCAPE '\\')");
                    args.push(Value::Text(format!("%/{name}%/%")));
                    args.push(Value::Text(format!("%\\\\{name}%\\\\%")));
                }
                Filter::Since(t) => {
                    sql.push_str(" AND s.mtime >= ?");
                    args.push(Value::Integer(*t));
                }
                Filter::Until(t) => {
                    sql.push_str(" AND s.mtime < ?");
                    args.push(Value::Integer(*t));
                }
            }
        }

        sql.push_str(&if long.is_empty() {
            " ORDER BY s.mtime DESC".to_string()
        } else {
            format!(" ORDER BY {alias}.rank, s.mtime DESC")
        });
        sql.push_str(" LIMIT ?");
        args.push(Value::Integer(limit as i64));

        let mut stmt = self.db.prepare_cached(&sql)?;
        let hits = stmt
            .query_map(params_from_iter(args), |r| {
                Ok(Hit {
                    id: r.get(0)?,
                    path: PathBuf::from(r.get::<_, String>(1)?),
                    mtime: r.get(2)?,
                    width: r.get(3)?,
                    height: r.get(4)?,
                    lines: Vec::new(),
                    near: false,
                    look: r.get::<_, Option<i64>>(5)?.map(|l| l as u64),
                    burst: r.get(6)?,
                })
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(hits)
    }

    /// The lines of one shot that contain any of the query's terms. For a
    /// near match, also the lines where one was misread, which is where the
    /// word sits even though it isn't spelled the way it was typed.
    pub fn matching_lines(&self, shot_id: i64, query: &str, near: bool) -> Result<Vec<Line>> {
        let needles: Vec<Vec<char>> = Query::parse(query).terms.iter().map(|t| lower(t)).collect();
        if needles.is_empty() {
            return Ok(Vec::new());
        }
        let mut lines = self.lines(shot_id)?;
        lines.retain(|l| {
            let text = lower(&l.text);
            needles
                .iter()
                .any(|n| reads_near(&text, n, if near { swaps(n) } else { 0 }))
        });
        Ok(lines)
    }

    pub fn lines(&self, shot_id: i64) -> Result<Vec<Line>> {
        read_lines(&self.db, shot_id)
    }
}

fn read_lines(db: &Connection, shot_id: i64) -> Result<Vec<Line>> {
    let mut stmt = db.prepare_cached(
        "SELECT text, x, y, w, h, score FROM lines WHERE shot_id = ?1 ORDER BY id",
    )?;
    let lines = stmt
        .query_map([shot_id], |r| {
            Ok(Line {
                text: r.get(0)?,
                rect: Rect {
                    x: r.get(1)?,
                    y: r.get(2)?,
                    w: r.get(3)?,
                    h: r.get(4)?,
                },
                score: r.get(5)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(lines)
}

/// Puts a shot in its burst: compares it with every settled shot taken
/// within `burst::WINDOW` of it, and joins the bursts of those it's alike
/// into one, named by the lowest id among them. Shots not settled yet are
/// left alone; they meet this one when their own turn comes.
fn settle(db: &Connection, id: i64) -> Result<()> {
    let me = db
        .query_row(
            "SELECT mtime, width, height, look FROM shots WHERE id = ?1 AND width > 0",
            [id],
            |r| {
                Ok((
                    r.get::<_, i64>(0)?,
                    r.get::<_, u32>(1)?,
                    r.get::<_, u32>(2)?,
                    r.get::<_, Option<i64>>(3)?,
                ))
            },
        )
        .optional()?;
    let Some((mtime, width, height, look)) = me else {
        return Ok(());
    };
    let me = Shape::new(width, height, look.map(|l| l as u64), &read_lines(db, id)?);

    let mut stmt = db.prepare_cached(
        "SELECT id, mtime, width, height, look, burst FROM shots
         WHERE width > 0 AND burst IS NOT NULL AND id != ?1 AND mtime BETWEEN ?2 AND ?3",
    )?;
    let near = stmt
        .query_map(params![id, mtime - WINDOW, mtime + WINDOW], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, u32>(2)?,
                r.get::<_, u32>(3)?,
                r.get::<_, Option<i64>>(4)?,
                r.get::<_, i64>(5)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut joined = Vec::new();
    for (other, when, w, h, look, burst) in near {
        if joined.contains(&burst) {
            continue;
        }
        let shape = Shape::new(w, h, look.map(|l| l as u64), &read_lines(db, other)?);
        if alike(&Likeness::of(&me, &shape), (mtime - when).abs()) {
            joined.push(burst);
        }
    }
    let name = joined.iter().copied().fold(id, i64::min);
    db.execute(
        "UPDATE shots SET burst = ?2 WHERE id = ?1",
        params![id, name],
    )?;
    for burst in joined.into_iter().filter(|b| *b != name) {
        db.execute(
            "UPDATE shots SET burst = ?2 WHERE burst = ?1",
            params![burst, name],
        )?;
    }
    Ok(())
}

fn delete(tx: &rusqlite::Transaction, path: &str) -> Result<bool> {
    let Some(id) = tx
        .query_row("SELECT id FROM shots WHERE path = ?1", [path], |r| {
            r.get::<_, i64>(0)
        })
        .optional()?
    else {
        return Ok(false);
    };
    // The fts table has no foreign key, so it has to be cleaned up by hand.
    tx.execute("DELETE FROM shots_fts WHERE rowid = ?1", [id])?;
    tx.execute("DELETE FROM shots WHERE id = ?1", [id])?;
    Ok(true)
}

// Each term becomes a quoted fts5 string, which turns off the query syntax
// inside it. Without this, typing `c++` or `"` or `NOT` would be a parse error
// or silently mean something else.
fn match_expr(terms: &[&str]) -> String {
    terms
        .iter()
        .map(|t| format!("\"{}\"", t.replace('"', "\"\"")))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Text the way OCR might have misread it, with every group of look-alikes
/// written the same way. Each character folds on its own, never in pairs,
/// so the fold of a word starts with the fold of every prefix of it and
/// search-as-you-type keeps finding the shot letter by letter. That's why
/// `m` becomes `rn` and not the other way round.
///
/// Spaces stay. Folding them away would find words OCR split (`ord er`),
/// but on a real index it mostly found words that were apart all along:
/// "in voice" for invoice, "e mail" for email, "setting speeds" for settings.
fn fold(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars().flat_map(char::to_lowercase) {
        if let Some(group) = look_alike(c) {
            out.push(TWINS[group][0]);
            continue;
        }
        match c {
            'm' => out.push_str("rn"),
            'w' => out.push_str("vv"),
            'd' => out.push_str("cl"),
            c => out.push(c),
        }
    }
    out
}

// Letters OCR takes for one another. The first of each is what `fold`
// writes for all of them.
const TWINS: [&[char]; 4] = [
    &['o', '0'],
    &['l', '1', 'i', '|'],
    &['s', '5', '$'],
    &['b', '8'],
];

// And the letters it reads as two, or two as one.
const PAIRS: [(char, [char; 2]); 3] = [('m', ['r', 'n']), ('w', ['v', 'v']), ('d', ['c', 'l'])];

fn look_alike(c: char) -> Option<usize> {
    TWINS.iter().position(|g| g.contains(&c))
}

/// How many misread letters a term may have and still be a near match.
/// None under four letters, where one swap is a third of the word ("hub"
/// found "shu849"), one up to seven, two from eight. Any more and ordinary
/// words start matching each other.
fn swaps(term: &[char]) -> usize {
    match term.len() {
        0..4 => 0,
        4..8 => 1,
        _ => 2,
    }
}

fn lower(text: &str) -> Vec<char> {
    text.chars().flat_map(char::to_lowercase).collect()
}

/// Whether `term` appears somewhere in `text` with at most `swaps` letters
/// read as their look-alikes. Both already lowercase.
fn reads_near(text: &[char], term: &[char], swaps: usize) -> bool {
    (0..text.len()).any(|i| reads_as(&text[i..], term, swaps))
}

fn reads_as(text: &[char], term: &[char], swaps: usize) -> bool {
    let Some(&want) = term.first() else {
        return true;
    };
    let Some(&got) = text.first() else {
        return false;
    };
    if got == want {
        return reads_as(&text[1..], &term[1..], swaps);
    }
    if swaps == 0 {
        return false;
    }
    if look_alike(got).is_some() && look_alike(got) == look_alike(want) {
        return reads_as(&text[1..], &term[1..], swaps - 1);
    }
    PAIRS.iter().any(|&(one, two)| {
        (want == one && text.starts_with(&two) && reads_as(&text[2..], &term[1..], swaps - 1))
            || (got == one && term.starts_with(&two) && reads_as(&text[1..], &term[2..], swaps - 1))
    })
}

fn escape_like(term: &str) -> String {
    term.replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

fn path_str(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn line(text: &str, y: f32) -> Line {
        Line {
            text: text.into(),
            rect: Rect {
                x: 0.1,
                y,
                w: 0.5,
                h: 0.03,
            },
            score: 0.9,
        }
    }

    fn shot(path: &str, mtime: i64) -> Shot {
        Shot {
            path: path.into(),
            mtime,
            width: 2568,
            height: 1428,
            look: None,
        }
    }

    fn sample() -> Index {
        let mut idx = Index::open_in_memory().unwrap();
        idx.insert(
            &shot("/shots/youtube.png", 100),
            &[
                line("IP: donutsmp.net", 0.2),
                line("Later in this video...", 0.5),
                line("Subscribe", 0.8),
            ],
        )
        .unwrap();
        idx.insert(
            &shot("/shots/pr.png", 200),
            &[
                line("add extension: clean-keyboard #288", 0.1),
                line("Uses EVIOCGRAB to grab", 0.6),
            ],
        )
        .unwrap();
        idx
    }

    fn paths(hits: &[Hit]) -> Vec<&str> {
        hits.iter().map(|h| h.path.to_str().unwrap()).collect()
    }

    // Lines as the Bengali reader gave them back from a screenshot of
    // Bengali Wikipedia. Same as Devanagari below: vowel signs and the
    // hasanta are code points of their own to the trigram index.
    #[test]
    fn finds_bengali() {
        let mut idx = Index::open_in_memory().unwrap();
        idx.insert(
            &shot("/shots/wiki.png", 100),
            &[
                line("দক্ষিণ এশিয়ার সার্বভৌম রাষ্ট্র", 0.1),
                line("বিচারে প্রায় ২০ কোটিরও অধিক জনসংখ্যা নিয়ে বাংলাদেশ", 0.2),
            ],
        )
        .unwrap();
        idx.insert(
            &shot("/shots/widget.png", 200),
            &[line("সূর্যোদয়: ভোর ৫:৫৩", 0.1), line("Play Store", 0.3)],
        )
        .unwrap();

        let found = |q: &str| paths(&idx.search(q, 10).unwrap()).join(" ");
        assert_eq!(found("বাংলাদেশ"), "/shots/wiki.png");
        assert_eq!(found("সূর্যোদয়"), "/shots/widget.png");
        // The middle of a word, through a conjunct.
        assert_eq!(found("ষ্ট্র"), "/shots/wiki.png");
        // Two words from different lines.
        assert_eq!(found("এশিয়ার জনসংখ্যা"), "/shots/wiki.png");
        // Bengali digits are not folded into Latin ones.
        assert_eq!(found("৫:৫৩"), "/shots/widget.png");
        assert_eq!(found("নেই"), "");
    }

    // Lines as the Devanagari reader gave them back from test screenshots.
    // Vowel signs and the virama are their own code points, which the trigram
    // index takes like any other character, and no case folding touches them.
    #[test]
    fn finds_devanagari() {
        let mut idx = Index::open_in_memory().unwrap();
        idx.insert(
            &shot("/shots/chat.png", 100),
            &[
                line("नमस्ते, आप कैसे हैं?", 0.1),
                line("कल मिलते हैं, शाम 6 बजे", 0.2),
            ],
        )
        .unwrap();
        idx.insert(
            &shot("/shots/order.png", 200),
            &[
                line("Order #4021 का स्टेटस", 0.1),
                line("भाषा: हिन्दी", 0.3),
                line("सूचना: उदा पाणीपुरवठा बंद राहील", 0.5),
            ],
        )
        .unwrap();

        let found = |q: &str| paths(&idx.search(q, 10).unwrap()).join(" ");
        assert_eq!(found("मिलते"), "/shots/chat.png");
        assert_eq!(found("हिन्दी"), "/shots/order.png");
        // The middle of a word, through a conjunct.
        assert_eq!(found("न्द"), "/shots/order.png");
        assert_eq!(found("पुरवठा"), "/shots/order.png");
        // Two characters is under the trigram index, so LIKE.
        assert_eq!(found("कल"), "/shots/chat.png");
        // Mixed with English, terms on different lines.
        assert_eq!(found("order हिन्दी"), "/shots/order.png");
        assert_eq!(found("स्टेटस"), "/shots/order.png");
        assert_eq!(found("नमस्कार"), "");

        let hits = idx.search("शाम", 10).unwrap();
        assert_eq!(hits[0].lines, [line("कल मिलते हैं, शाम 6 बजे", 0.2)]);
    }

    #[test]
    fn finds_the_middle_of_a_word() {
        let hits = sample().search("nutsmp", 10).unwrap();
        assert_eq!(paths(&hits), ["/shots/youtube.png"]);
        assert_eq!(hits[0].lines, [line("IP: donutsmp.net", 0.2)]);
    }

    #[test]
    fn is_case_insensitive() {
        assert_eq!(
            paths(&sample().search("eviocgrab", 10).unwrap()),
            ["/shots/pr.png"]
        );
    }

    #[test]
    fn terms_can_be_on_different_lines() {
        let hits = sample().search("subscribe later", 10).unwrap();
        assert_eq!(paths(&hits), ["/shots/youtube.png"]);
        assert_eq!(hits[0].lines.len(), 2);
    }

    #[test]
    fn every_term_must_match() {
        assert!(
            sample()
                .search("subscribe eviocgrab", 10)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn short_terms_fall_back_to_like() {
        assert_eq!(
            paths(&sample().search("ip", 10).unwrap()),
            ["/shots/youtube.png"]
        );
        assert_eq!(
            paths(&sample().search("#2", 10).unwrap()),
            ["/shots/pr.png"]
        );
        assert_eq!(
            paths(&sample().search("ip donut", 10).unwrap()),
            ["/shots/youtube.png"]
        );
    }

    #[test]
    fn query_syntax_is_not_interpreted() {
        let idx = sample();
        for q in ["\"", "NOT", "c++", "a*", "100%", "_", "(", "clean-keyboard"] {
            idx.search(q, 10).unwrap();
        }
        assert_eq!(
            paths(&idx.search("clean-keyboard", 10).unwrap()),
            ["/shots/pr.png"]
        );
        assert!(idx.search("100%", 10).unwrap().is_empty());
    }

    #[test]
    fn empty_query_is_newest_first() {
        let hits = sample().search("   ", 10).unwrap();
        assert_eq!(paths(&hits), ["/shots/pr.png", "/shots/youtube.png"]);
        assert!(hits.iter().all(|h| h.lines.is_empty()));
    }

    #[test]
    fn reinserting_a_path_replaces_it() {
        let mut idx = sample();
        idx.insert(
            &shot("/shots/youtube.png", 300),
            &[line("something else", 0.1)],
        )
        .unwrap();
        assert_eq!(idx.len().unwrap(), 2);
        assert!(idx.search("donutsmp", 10).unwrap().is_empty());
        assert_eq!(
            paths(&idx.search("something", 10).unwrap()),
            ["/shots/youtube.png"]
        );
        assert!(
            idx.is_current(Path::new("/shots/youtube.png"), 300)
                .unwrap()
        );
        assert!(
            !idx.is_current(Path::new("/shots/youtube.png"), 100)
                .unwrap()
        );
    }

    #[test]
    fn a_shot_with_no_text_is_still_indexed() {
        let mut idx = Index::open_in_memory().unwrap();
        idx.insert(&shot("/shots/blank.png", 1), &[]).unwrap();
        assert_eq!(idx.len().unwrap(), 1);
        assert_eq!(idx.search("", 10).unwrap().len(), 1);
        assert!(idx.search("anything", 10).unwrap().is_empty());
    }

    #[test]
    fn removed_shots_stop_matching() {
        let mut idx = sample();
        assert!(idx.remove(Path::new("/shots/youtube.png")).unwrap());
        assert!(!idx.remove(Path::new("/shots/youtube.png")).unwrap());
        assert!(idx.search("donutsmp", 10).unwrap().is_empty());
        assert_eq!(idx.len().unwrap(), 1);
    }

    #[test]
    fn rename_keeps_the_text() {
        let mut idx = sample();
        assert!(
            idx.rename(
                Path::new("/shots/youtube.png"),
                Path::new("/shots/moved.png")
            )
            .unwrap()
        );
        assert_eq!(
            paths(&idx.search("donutsmp", 10).unwrap()),
            ["/shots/moved.png"]
        );
        assert!(idx.is_current(Path::new("/shots/moved.png"), 100).unwrap());
        assert!(
            !idx.rename(Path::new("/shots/youtube.png"), Path::new("/x.png"))
                .unwrap()
        );
    }

    #[test]
    fn rename_onto_an_existing_shot_replaces_it() {
        let mut idx = sample();
        idx.rename(Path::new("/shots/youtube.png"), Path::new("/shots/pr.png"))
            .unwrap();
        assert_eq!(idx.len().unwrap(), 1);
        assert_eq!(
            paths(&idx.search("donutsmp", 10).unwrap()),
            ["/shots/pr.png"]
        );
        assert!(idx.search("eviocgrab", 10).unwrap().is_empty());
    }

    #[test]
    fn prune_drops_files_that_no_longer_exist() {
        let dir = std::env::temp_dir().join(format!("gyotaku-prune-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let kept = dir.join("kept.png");
        std::fs::write(&kept, b"").unwrap();

        let mut idx = Index::open_in_memory().unwrap();
        idx.insert(
            &Shot {
                path: kept.clone(),
                mtime: 1,
                width: 10,
                height: 10,
                look: None,
            },
            &[],
        )
        .unwrap();
        idx.insert(&shot("/definitely/not/here.png", 1), &[])
            .unwrap();
        assert_eq!(idx.prune().unwrap(), 1);
        assert_eq!(idx.paths().unwrap(), [kept]);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn zero_sized_shots_are_indexed_but_hidden() {
        let mut idx = sample();
        let broken = Shot {
            path: "/shots/broken.png".into(),
            mtime: 999,
            width: 0,
            height: 0,
            look: None,
        };
        idx.insert(&broken, &[line("donutsmp", 0.1)]).unwrap();
        assert!(idx.is_current(Path::new("/shots/broken.png"), 999).unwrap());
        assert_eq!(idx.len().unwrap(), 3);
        assert_eq!(idx.visible_len().unwrap(), 2);
        assert_eq!(
            paths(&idx.search("donutsmp", 10).unwrap()),
            ["/shots/youtube.png"]
        );
        assert_eq!(idx.search("", 10).unwrap().len(), 2);
    }

    #[test]
    fn find_is_search_without_the_lines() {
        let idx = sample();
        let found = idx.find("subscribe later", 10).unwrap();
        assert_eq!(paths(&found), ["/shots/youtube.png"]);
        assert!(found[0].lines.is_empty());
        let lines = idx
            .matching_lines(found[0].id, "subscribe later", false)
            .unwrap();
        assert_eq!(lines, idx.search("subscribe later", 10).unwrap()[0].lines);
        assert!(
            idx.matching_lines(found[0].id, "  ", false)
                .unwrap()
                .is_empty()
        );
    }

    fn misread() -> Index {
        let mut idx = Index::open_in_memory().unwrap();
        idx.insert(
            &shot("/shots/caps.png", 100),
            &[line("YOUR 0RDER HAS SHIPPED", 0.2)],
        )
        .unwrap();
        idx.insert(
            &shot("/shots/serif.png", 200),
            &[line("Thanks for your orcler", 0.3)],
        )
        .unwrap();
        idx.insert(
            &shot("/shots/discord.png", 250),
            &[line("HOURS IN VOICE", 0.3)],
        )
        .unwrap();
        idx.insert(
            &shot("/shots/blog.png", 300),
            &[line("a rnodern take on tea", 0.4)],
        )
        .unwrap();
        idx.insert(&shot("/shots/clean.png", 50), &[line("Order #4021", 0.1)])
            .unwrap();
        idx.insert(
            &shot("/shots/rust.png", 400),
            &[line("error[E0425]: cannot find", 0.1)],
        )
        .unwrap();
        idx.insert(
            &shot("/shots/ocr.png", 500),
            &[line("error[EO425]: cannot find", 0.1)],
        )
        .unwrap();
        idx
    }

    fn near(hits: &[Hit]) -> Vec<(&str, bool)> {
        hits.iter()
            .map(|h| (h.path.to_str().unwrap(), h.near))
            .collect()
    }

    #[test]
    fn look_alikes_are_near_matches() {
        let hits = misread().search("order", 10).unwrap();
        assert_eq!(
            near(&hits),
            [
                ("/shots/clean.png", false),
                ("/shots/serif.png", true),
                ("/shots/caps.png", true),
            ]
        );
        // The lines lit are the ones as they were read.
        assert_eq!(hits[1].lines, [line("Thanks for your orcler", 0.3)]);
        assert_eq!(hits[2].lines, [line("YOUR 0RDER HAS SHIPPED", 0.2)]);
    }

    #[test]
    fn words_apart_stay_apart() {
        assert!(misread().search("invoice", 10).unwrap().is_empty());
    }

    // Lines from a real index, the near matches the first try found.
    #[test]
    fn one_misread_letter_is_near_but_two_are_another_word() {
        let mut idx = Index::open_in_memory().unwrap();
        let lines = [
            "Elevated errors across many modeis",
            "\"Flles changed",
            "C0NF1GURATI0N",
            "Starting point was that gradientmap supports 32 stops internally",
            "Butterflies!!!",
            "Money Forward Interview!!!!",
            "CHLO NAALOO DE PARONTHE",
        ];
        for (i, text) in lines.iter().enumerate() {
            idx.insert(
                &shot(&format!("/shots/{i}.png"), i as i64),
                &[line(text, 0.1)],
            )
            .unwrap();
        }
        let found = |q: &str| paths(&idx.search(q, 10).unwrap()).join(" ");
        assert_eq!(found("model"), "/shots/0.png");
        assert_eq!(found("file"), "/shots/1.png");
        // Two swaps are fine from eight letters up.
        assert_eq!(found("configuration"), "");
        assert_eq!(found("configurati0n"), "/shots/2.png");
        assert_eq!(found("email"), "");
        assert_eq!(found("will"), "");
        assert_eq!(found("100"), "");
    }

    #[test]
    fn rn_reads_as_m_and_the_other_way_round() {
        let idx = misread();
        assert_eq!(
            near(&idx.search("modern", 10).unwrap()),
            [("/shots/blog.png", true)]
        );
        assert_eq!(
            near(&idx.search("rnodern", 10).unwrap()),
            [("/shots/blog.png", false)]
        );
    }

    #[test]
    fn near_matches_keep_up_while_typing() {
        let idx = misread();
        // Three letters are too few to guess a misreading from.
        assert!(idx.find("mod", 10).unwrap().is_empty());
        for typed in ["mode", "moder", "modern"] {
            assert_eq!(
                paths(&idx.find(typed, 10).unwrap()),
                ["/shots/blog.png"],
                "{typed}"
            );
        }
    }

    #[test]
    fn an_exact_code_stays_ahead_of_its_misreading() {
        let hits = misread().search("E0425", 10).unwrap();
        assert_eq!(
            near(&hits),
            [("/shots/rust.png", false), ("/shots/ocr.png", true)]
        );
        let hits = misread().search("EO425", 10).unwrap();
        assert_eq!(
            near(&hits),
            [("/shots/ocr.png", false), ("/shots/rust.png", true)]
        );
    }

    #[test]
    fn exact_matches_that_fill_the_limit_leave_no_room_for_near() {
        let idx = misread();
        assert_eq!(
            near(&idx.find("order", 1).unwrap()),
            [("/shots/clean.png", false)]
        );
        assert_eq!(idx.find("order", 2).unwrap().len(), 2);
    }

    #[test]
    fn short_queries_have_no_near_matches() {
        let idx = misread();
        // "0r" only exists exactly in caps.png; folded it would also be in
        // every "or".
        assert_eq!(
            near(&idx.search("0r", 10).unwrap()),
            [("/shots/caps.png", false)]
        );
        // A short term next to a long one still has to be there as read.
        assert_eq!(
            near(&idx.search("order #4", 10).unwrap()),
            [("/shots/clean.png", false)]
        );
        assert_eq!(
            near(&idx.search("order er", 10).unwrap()),
            [
                ("/shots/clean.png", false),
                ("/shots/serif.png", true),
                ("/shots/caps.png", true),
            ]
        );
    }

    #[test]
    fn removing_a_shot_removes_its_folded_text() {
        let mut idx = misread();
        idx.remove(Path::new("/shots/serif.png")).unwrap();
        let left: i64 = idx
            .db
            .query_row("SELECT count(*) FROM shots_near", [], |r| r.get(0))
            .unwrap();
        assert_eq!(left, 6);
        assert!(!paths(&idx.find("order", 10).unwrap()).contains(&"/shots/serif.png"));
    }

    #[test]
    fn a_version_1_index_is_brought_forward_not_rebuilt() {
        let dir = std::env::temp_dir().join(format!("gyotaku-v1-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("index.db");
        {
            let db = Connection::open(&path).unwrap();
            db.execute_batch(TABLES).unwrap();
            db.execute_batch(
                "INSERT INTO shots (id, path, mtime, width, height)
                     VALUES (7, '/shots/old.png', 1, 100, 100);
                 INSERT INTO lines (shot_id, text, x, y, w, h, score)
                     VALUES (7, 'YOUR 0RDER', 0.1, 0.1, 0.5, 0.03, 0.9);
                 INSERT INTO shots_fts (rowid, text) VALUES (7, 'YOUR 0RDER');
                 PRAGMA user_version = 1;",
            )
            .unwrap();
        }
        let idx = Index::open(&path).unwrap();
        assert_eq!(idx.len().unwrap(), 1);
        assert_eq!(
            near(&idx.search("order", 10).unwrap()),
            [("/shots/old.png", true)]
        );
        assert_eq!(
            near(&idx.search("0rder", 10).unwrap()),
            [("/shots/old.png", false)]
        );
        // Read before bursts existed, so it waits for the reader to settle it.
        assert_eq!(
            idx.unsettled(10).unwrap(),
            [(7, PathBuf::from("/shots/old.png"), false)]
        );
        drop(idx);
        // And opening it again leaves it be.
        let mut idx = Index::open(&path).unwrap();
        assert_eq!(idx.len().unwrap(), 1);
        idx.settle(7, Some(42)).unwrap();
        assert!(idx.unsettled(10).unwrap().is_empty());
        let hit = &idx.find("", 10).unwrap()[0];
        assert_eq!((hit.look, hit.burst), (Some(42), 7));
        // Closed first: Windows won't delete a file that's still open.
        drop(idx);
        std::fs::remove_dir_all(&dir).unwrap();
    }

    fn page(words: &[&str]) -> Vec<Line> {
        words
            .iter()
            .enumerate()
            .map(|(i, w)| line(w, 0.1 + i as f32 * 0.1))
            .collect()
    }

    const PAGE: [&str; 4] = [
        "Your order has shipped",
        "Arriving on Thursday",
        "Track your package",
        "Order number 4021",
    ];

    fn burst_of(idx: &Index, path: &str) -> i64 {
        idx.find("", 100)
            .unwrap()
            .into_iter()
            .find(|h| h.path.to_str() == Some(path))
            .unwrap()
            .burst
    }

    #[test]
    fn the_same_page_twice_is_one_burst() {
        let mut idx = Index::open_in_memory().unwrap();
        idx.insert(&shot("/a.png", 1000), &page(&PAGE)).unwrap();
        idx.insert(&shot("/b.png", 1030), &page(&PAGE)).unwrap();
        idx.insert(
            &shot("/c.png", 1060),
            &page(&["Something else", "entirely different", "on this page"]),
        )
        .unwrap();
        // Too long after to be the same burst, however alike.
        idx.insert(&shot("/d.png", 1000 + 2 * WINDOW), &page(&PAGE))
            .unwrap();
        let (a, b, c, d) = (
            burst_of(&idx, "/a.png"),
            burst_of(&idx, "/b.png"),
            burst_of(&idx, "/c.png"),
            burst_of(&idx, "/d.png"),
        );
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert_ne!(a, d);
    }

    #[test]
    fn a_shot_between_two_bursts_joins_them() {
        let mut idx = Index::open_in_memory().unwrap();
        idx.insert(&shot("/first.png", 0), &page(&PAGE)).unwrap();
        idx.insert(&shot("/last.png", WINDOW + 100), &page(&PAGE))
            .unwrap();
        assert_ne!(burst_of(&idx, "/first.png"), burst_of(&idx, "/last.png"));
        // Taken in the middle, within reach of both.
        idx.insert(&shot("/middle.png", WINDOW / 2 + 50), &page(&PAGE))
            .unwrap();
        let first = burst_of(&idx, "/first.png");
        assert_eq!(first, burst_of(&idx, "/middle.png"));
        assert_eq!(first, burst_of(&idx, "/last.png"));
        // Named by the lowest id, which is the first one read.
        assert_eq!(
            first,
            idx.find("", 10)
                .unwrap()
                .iter()
                .map(|h| h.id)
                .min()
                .unwrap()
        );
    }

    #[test]
    fn hidden_shots_are_never_settled() {
        let mut idx = Index::open_in_memory().unwrap();
        let broken = Shot {
            look: None,
            width: 0,
            height: 0,
            ..shot("/broken.png", 5)
        };
        idx.insert(&broken, &[]).unwrap();
        assert!(idx.unsettled(10).unwrap().is_empty());
    }

    #[test]
    fn fold_keeps_spaces_lines_and_prefixes() {
        assert_eq!(fold("ord\ner"), "orcl\ner");
        assert_eq!(fold("Wi-Fi 5G"), "vvl-fl sg");
        let word = fold("modern");
        for end in 1..="modern".len() {
            assert!(word.starts_with(&fold(&"modern"[..end])));
        }
    }

    #[test]
    fn limit_zero_returns_nothing() {
        assert!(sample().search("", 0).unwrap().is_empty());
    }

    // Noon UTC, so the day is the same in every timezone the tests run in.
    fn noon(date: &str) -> i64 {
        format!("{date}T12:00:00Z")
            .parse::<jiff::Timestamp>()
            .unwrap()
            .as_second()
    }

    fn filed() -> Index {
        let mut idx = Index::open_in_memory().unwrap();
        let now = jiff::Timestamp::now().as_second();
        for (path, mtime, text) in [
            (
                "/home/me/Pictures/Discord/otp.png",
                noon("2026-08-15"),
                "your code is 4821",
            ),
            (
                "/home/me/Pictures/Screenshots/otp.png",
                noon("2026-07-02"),
                "otp 9912 code",
            ),
            (
                "/home/me/Pictures/Screenshots/discord.png",
                now,
                "discord code",
            ),
            (
                "C:\\Users\\me\\Pictures\\Discord\\win.png",
                noon("2026-08-20"),
                "code on windows",
            ),
        ] {
            idx.insert(&shot(path, mtime), &[line(text, 0.5)]).unwrap();
        }
        idx
    }

    #[test]
    fn in_matches_folders_not_file_names() {
        let hits = filed().search("code in:disc", 10).unwrap();
        assert_eq!(
            paths(&hits),
            [
                "C:\\Users\\me\\Pictures\\Discord\\win.png",
                "/home/me/Pictures/Discord/otp.png"
            ]
        );
        // The lit lines come from the words, never from the filter.
        assert_eq!(hits[1].lines, [line("your code is 4821", 0.5)]);
    }

    #[test]
    fn dates_narrow_by_when_it_was_taken() {
        let idx = filed();
        assert_eq!(
            paths(&idx.search("code date:2026-08-15", 10).unwrap()),
            ["/home/me/Pictures/Discord/otp.png"]
        );
        assert_eq!(
            paths(&idx.search("code before:2026-08", 10).unwrap()),
            ["/home/me/Pictures/Screenshots/otp.png"]
        );
        assert_eq!(
            paths(&idx.search("code date:today", 10).unwrap()),
            ["/home/me/Pictures/Screenshots/discord.png"]
        );
    }

    #[test]
    fn only_filters_lists_newest_first() {
        let hits = filed().search("in:pictures after:2026-08", 10).unwrap();
        assert_eq!(
            paths(&hits),
            [
                "/home/me/Pictures/Screenshots/discord.png",
                "C:\\Users\\me\\Pictures\\Discord\\win.png",
                "/home/me/Pictures/Discord/otp.png",
            ]
        );
        assert!(hits.iter().all(|h| h.lines.is_empty()));
    }

    #[test]
    fn filters_narrow_near_matches_too() {
        let mut idx = filed();
        idx.insert(
            &shot("/home/me/Pictures/Discord/misread.png", noon("2026-08-16")),
            &[line("c0de sent", 0.5)],
        )
        .unwrap();
        idx.insert(
            &shot("/home/me/Pictures/Other/misread.png", noon("2026-08-16")),
            &[line("c0de sent", 0.5)],
        )
        .unwrap();
        let hits = idx.search("code in:discord date:2026-08", 10).unwrap();
        let near: Vec<_> = hits
            .iter()
            .filter(|h| h.near)
            .map(|h| h.path.to_str().unwrap())
            .collect();
        assert_eq!(near, ["/home/me/Pictures/Discord/misread.png"]);
    }
}
