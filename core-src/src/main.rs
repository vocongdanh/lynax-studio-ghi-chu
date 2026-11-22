//! The core of the sample Studio "Ghi chú". It owns the notes, publishes its
//! actions as MCP tools over stdio, keeps an undo history, and turns the
//! agent's rename requests into proposals that only the person can accept.
//!
//! MCP over stdio is newline-delimited JSON-RPC. Desktop starts the core in
//! the operating system sandbox and hands it its stores in
//! `LYNAX_STUDIO_STORES`; the schema of the `main` store comes from the
//! package's migrations, which Desktop runs before the core starts.
use rusqlite::{Connection, OptionalExtension, params};
use serde_json::{Value, json};
use std::io::{BufRead, Write};

const MAX_NOTES: i64 = 500;
const MAX_HISTORY: i64 = 50;

fn tools() -> Value {
    let action = |name: &str, title: &str, description: &str, schema: Value, read_only: bool, meta: Value| {
        json!({"name": name, "description": description, "inputSchema": schema,
               "annotations": {"title": title, "readOnlyHint": read_only}, "_meta": meta})
    };
    let none = json!({"type": "object", "properties": {}, "additionalProperties": false});
    let proposal_id = json!({"type": "object", "required": ["proposal_id"], "additionalProperties": false,
        "properties": {"proposal_id": {"type": "string", "title": "Đề xuất", "maxLength": 64}}});
    json!([
        // The context action: Desktop runs it when a request starts, so the
        // agent already knows the notes and pending proposals.
        action("notes.list", "Xem ghi chú", "Liệt kê mọi ghi chú, các đề xuất đang chờ và số phiên bản hiện tại.", none.clone(), true,
            json!({"lynax.studio/confirm": "none", "lynax.studio/context": true})),
        action("notes.add", "Thêm ghi chú", "Thêm một ghi chú mới với tiêu đề cho trước.",
            json!({"type": "object", "required": ["title"], "additionalProperties": false,
                   "properties": {"title": {"type": "string", "title": "Tiêu đề", "minLength": 1, "maxLength": 200}}}),
            false, json!({"lynax.studio/confirm": "approval", "lynax.studio/undoable": true,
                          "lynax.studio/examples": [{"title": "Ghi chú thử"}]})),
        action("notes.propose_rename", "Đề xuất đổi tên",
            "Đề xuất đổi tên một ghi chú. Không đổi gì cho tới khi người dùng chấp nhận trong Studio.",
            json!({"type": "object", "required": ["note_id", "title"], "additionalProperties": false,
                   "properties": {"note_id": {"type": "string", "title": "Ghi chú", "maxLength": 64},
                                  "title": {"type": "string", "title": "Tên mới", "minLength": 1, "maxLength": 200}}}),
            true, json!({"lynax.studio/confirm": "proposal"})),
        action("proposals.list", "Xem đề xuất", "Liệt kê các đề xuất đang chờ người dùng.", none.clone(), true,
            json!({"lynax.studio/confirm": "none"})),
        action("proposals.accept", "Chấp nhận đề xuất", "Áp dụng một đề xuất.", proposal_id.clone(), false,
            json!({"lynax.studio/confirm": "approval", "lynax.studio/undoable": true, "lynax.studio/caller": "user"})),
        action("proposals.dismiss", "Bỏ đề xuất", "Bỏ một đề xuất.", proposal_id, false,
            json!({"lynax.studio/confirm": "approval", "lynax.studio/caller": "user"})),
        // Tried in this order by `studio.validate`: undo the example note, then redo it.
        action("history.undo", "Hoàn tác", "Hoàn tác thay đổi gần nhất.", none.clone(), false,
            json!({"lynax.studio/confirm": "approval", "lynax.studio/examples": [{}]})),
        action("history.redo", "Làm lại", "Làm lại thay đổi vừa hoàn tác.", none.clone(), false,
            json!({"lynax.studio/confirm": "approval", "lynax.studio/examples": [{}]})),
        // Writes outside the Studio, into the folder the person granted: no undo.
        action("notes.export", "Xuất ghi chú", "Ghi mọi ghi chú vào ghi-chu.md trong thư mục xuất người dùng đã chọn.", none, false,
            json!({"lynax.studio/confirm": "approval"})),
    ])
}

struct Notes {
    db: Connection,
}

type Outcome = Result<Value, String>;

fn fail(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// What Desktop sends with each call: who acts, the language to answer in,
/// and the values the person set for this Studio.
struct Ctx {
    caller: String,
    en: bool,
    by_title: bool,
}

impl Ctx {
    fn of(meta: &Value) -> Self {
        Self {
            // Without it, assume the agent.
            caller: meta["lynax.studio/caller"].as_str().unwrap_or("agent").to_owned(),
            en: meta["lynax.studio/locale"].as_str().is_some_and(|l| l.starts_with("en")),
            by_title: meta["lynax.studio/settings"]["sort"] == "title",
        }
    }

    fn t(&self, vi: &'static str, en: &'static str) -> &'static str {
        if self.en { en } else { vi }
    }
}

impl Notes {
    fn open() -> Result<Self, String> {
        let stores: Value = serde_json::from_str(&std::env::var("LYNAX_STUDIO_STORES").map_err(fail)?).map_err(fail)?;
        let path = stores["main"]["path"].as_str().ok_or("Thiếu kho main.")?;
        let db = Connection::open(path).map_err(fail)?;
        db.busy_timeout(std::time::Duration::from_secs(5)).map_err(fail)?;
        Ok(Self { db })
    }

    fn counter(&self, key: &str) -> Result<i64, String> {
        self.db.query_row("SELECT value FROM counters WHERE key=?1", [key], |r| r.get(0)).map_err(fail)
    }

    /// The notes in the order the person chose in the Studio's settings.
    fn notes(&self, ctx: &Ctx) -> Result<Value, String> {
        self.ordered(ctx.by_title)
    }

    fn ordered(&self, by_title: bool) -> Result<Value, String> {
        let order = if by_title { "title COLLATE NOCASE, position" } else { "position" };
        let mut statement = self.db.prepare(&format!("SELECT id, title FROM notes ORDER BY {order}")).map_err(fail)?;
        let rows = statement
            .query_map([], |r| Ok(json!({"id": r.get::<_, String>(0)?, "title": r.get::<_, String>(1)?})))
            .map_err(fail)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(fail)?;
        Ok(json!(rows))
    }

    /// The notes in their own order, as undo puts them back.
    fn snapshot(&self) -> Result<String, String> {
        Ok(self.ordered(false)?.to_string())
    }

    fn put_back(tx: &rusqlite::Transaction<'_>, snapshot: &str) -> Result<(), String> {
        let notes: Vec<Value> = serde_json::from_str(snapshot).map_err(fail)?;
        tx.execute("DELETE FROM notes", []).map_err(fail)?;
        for (position, note) in notes.iter().enumerate() {
            tx.execute(
                "INSERT INTO notes(id, title, position, created_by) VALUES(?1, ?2, ?3, 'history')",
                params![note["id"].as_str(), note["title"].as_str(), position as i64],
            )
            .map_err(fail)?;
        }
        Ok(())
    }

    fn next_id(tx: &rusqlite::Transaction<'_>, prefix: &str) -> Result<String, String> {
        let next: i64 = tx.query_row("SELECT value FROM counters WHERE key='next_id'", [], |r| r.get(0)).map_err(fail)?;
        tx.execute("UPDATE counters SET value=value+1 WHERE key='next_id'", []).map_err(fail)?;
        Ok(format!("{prefix}{next}"))
    }

    /// Bumps the revision inside the change's transaction and returns it.
    fn bump(tx: &rusqlite::Transaction<'_>) -> Result<i64, String> {
        tx.execute("UPDATE counters SET value=value+1 WHERE key='revision'", []).map_err(fail)?;
        tx.query_row("SELECT value FROM counters WHERE key='revision'", [], |r| r.get(0)).map_err(fail)
    }

    /// Keeps what the notes were before a change, so it can be undone.
    fn remember(tx: &rusqlite::Transaction<'_>, snapshot: &str) -> Result<(), String> {
        tx.execute("INSERT INTO history(stack, snapshot) VALUES('undo', ?1)", [snapshot]).map_err(fail)?;
        tx.execute("DELETE FROM history WHERE stack='redo'", []).map_err(fail)?;
        tx.execute(
            "DELETE FROM history WHERE stack='undo' AND id NOT IN (SELECT id FROM history WHERE stack='undo' ORDER BY id DESC LIMIT ?1)",
            [MAX_HISTORY],
        )
        .map_err(fail)?;
        Ok(())
    }

    /// Writes the notes to `ghi-chu.md` in the folder the person granted as
    /// `exports`; Desktop hands it over in `LYNAX_STUDIO_FOLDERS`.
    fn export(&self, ctx: &Ctx) -> Outcome {
        let folders: Value = std::env::var("LYNAX_STUDIO_FOLDERS").ok().and_then(|v| serde_json::from_str(&v).ok()).unwrap_or_default();
        let folder = folders["exports"]["path"].as_str().ok_or(ctx.t(
            "Chưa chọn thư mục xuất. Mở Cài đặt của Studio để chọn.",
            "No export folder yet. Choose one in the Studio's settings.",
        ))?;
        let notes = self.ordered(ctx.by_title)?;
        let mut text = format!("# {}\n\n", ctx.t("Ghi chú", "Notes"));
        for note in notes.as_array().into_iter().flatten() {
            text.push_str(&format!("- {}\n", note["title"].as_str().unwrap_or_default()));
        }
        std::fs::write(std::path::Path::new(folder).join("ghi-chu.md"), text).map_err(fail)?;
        let count = notes.as_array().map_or(0, Vec::len);
        Ok(json!({"file": "ghi-chu.md", "count": count,
                  "summary": if ctx.en { format!("Exported {count} notes") } else { format!("Đã xuất {count} ghi chú") }}))
    }

    fn call(&mut self, name: &str, args: &Value, ctx: &Ctx) -> Outcome {
        match name {
            "notes.export" => self.export(ctx),
            "notes.list" => Ok(json!({"revision": self.counter("revision")?, "notes": self.notes(ctx)?, "proposals": self.proposals()?})),
            "proposals.list" => Ok(json!({"revision": self.counter("revision")?, "proposals": self.proposals()?})),
            _ => self.change(name, args, ctx),
        }
    }

    fn proposals(&self) -> Result<Value, String> {
        let mut statement = self.db.prepare("SELECT id, note_id, title, summary, revision FROM proposals ORDER BY rowid").map_err(fail)?;
        let rows = statement
            .query_map([], |r| {
                Ok(json!({"id": r.get::<_, String>(0)?, "note_id": r.get::<_, String>(1)?, "title": r.get::<_, String>(2)?,
                          "summary": r.get::<_, String>(3)?, "revision": r.get::<_, i64>(4)?}))
            })
            .map_err(fail)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(fail)?;
        Ok(json!(rows))
    }

    /// Every change and every proposal returns the notes as they are now, so
    /// whoever asked does not need to read them again to see the result (a
    /// proposal shows the notes unchanged).
    fn change(&mut self, name: &str, args: &Value, ctx: &Ctx) -> Outcome {
        let mut result = self.write(name, args, ctx)?;
        result["notes"] = self.notes(ctx)?;
        Ok(result)
    }

    fn write(&mut self, name: &str, args: &Value, ctx: &Ctx) -> Outcome {
        let caller = ctx.caller.as_str();
        match name {
            "notes.add" => {
                let title = text(args, "title")?;
                let snapshot = self.snapshot()?;
                let tx = self.db.transaction().map_err(fail)?;
                let count: i64 = tx.query_row("SELECT COUNT(*) FROM notes", [], |r| r.get(0)).map_err(fail)?;
                if count >= MAX_NOTES {
                    return Err(ctx.t("Đã đủ số ghi chú tối đa.", "The notes are at their limit.").into());
                }
                Self::remember(&tx, &snapshot)?;
                let id = Self::next_id(&tx, "n")?;
                tx.execute("INSERT INTO notes(id, title, position, created_by) VALUES(?1, ?2, ?3, ?4)", params![id, title, count, caller])
                    .map_err(fail)?;
                let revision = Self::bump(&tx)?;
                tx.commit().map_err(fail)?;
                Ok(json!({"revision": revision, "summary": if ctx.en { format!("Added the note “{title}”") } else { format!("Đã thêm ghi chú “{title}”") }, "note_id": id}))
            }
            "notes.propose_rename" => {
                let note = text(args, "note_id")?;
                let title = text(args, "title")?;
                let current: String = self
                    .db
                    .query_row("SELECT title FROM notes WHERE id=?1", [&note], |r| r.get(0))
                    .optional()
                    .map_err(fail)?
                    .ok_or(ctx.t("Không có ghi chú này.", "There is no such note."))?;
                let tx = self.db.transaction().map_err(fail)?;
                let id = Self::next_id(&tx, "p")?;
                let summary = if ctx.en { format!("Rename “{current}” to “{title}”") } else { format!("Đổi “{current}” thành “{title}”") };
                let revision: i64 = tx.query_row("SELECT value FROM counters WHERE key='revision'", [], |r| r.get(0)).map_err(fail)?;
                // A proposal is kept with the notes but changes no note.
                tx.execute("INSERT INTO proposals(id, note_id, title, summary, revision) VALUES(?1, ?2, ?3, ?4, ?5)",
                    params![id, note, title, summary, revision]).map_err(fail)?;
                tx.commit().map_err(fail)?;
                Ok(json!({"proposal": {"id": id, "note_id": note, "title": title, "summary": summary, "revision": revision}}))
            }
            "proposals.accept" | "proposals.dismiss" => {
                let id = text(args, "proposal_id")?;
                let snapshot = self.snapshot()?;
                let tx = self.db.transaction().map_err(fail)?;
                let proposal: Option<(String, String, String, i64)> = tx
                    .query_row("SELECT note_id, title, summary, revision FROM proposals WHERE id=?1", [&id], |r| {
                        Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
                    })
                    .optional()
                    .map_err(fail)?;
                let (note, title, summary, made_at) = proposal.ok_or(ctx.t("Không còn đề xuất này.", "This proposal is gone."))?;
                tx.execute("DELETE FROM proposals WHERE id=?1", [&id]).map_err(fail)?;
                let revision: i64 = tx.query_row("SELECT value FROM counters WHERE key='revision'", [], |r| r.get(0)).map_err(fail)?;
                if name == "proposals.dismiss" {
                    tx.commit().map_err(fail)?;
                    return Ok(json!({"revision": revision, "summary": ctx.t("Đã bỏ đề xuất", "Dismissed the proposal")}));
                }
                // A proposal is made against one revision; it expires when the notes change.
                if made_at != revision {
                    tx.commit().map_err(fail)?;
                    return Err(ctx.t("Đề xuất đã cũ vì ghi chú vừa thay đổi.", "The proposal is out of date: the notes changed.").into());
                }
                Self::remember(&tx, &snapshot)?;
                tx.execute("UPDATE notes SET title=?2 WHERE id=?1", params![note, title]).map_err(fail)?;
                let revision = Self::bump(&tx)?;
                tx.commit().map_err(fail)?;
                Ok(json!({"revision": revision, "summary": summary}))
            }
            "history.undo" | "history.redo" => {
                let (from, to) = if name == "history.undo" { ("undo", "redo") } else { ("redo", "undo") };
                let snapshot = self.snapshot()?;
                let tx = self.db.transaction().map_err(fail)?;
                let previous: Option<(i64, String)> = tx
                    .query_row("SELECT id, snapshot FROM history WHERE stack=?1 ORDER BY id DESC LIMIT 1", [from], |r| Ok((r.get(0)?, r.get(1)?)))
                    .optional()
                    .map_err(fail)?;
                let (row, previous) = previous.ok_or(ctx.t("Không có gì để thực hiện.", "There is nothing to do."))?;
                tx.execute("DELETE FROM history WHERE id=?1", [row]).map_err(fail)?;
                tx.execute("INSERT INTO history(stack, snapshot) VALUES(?1, ?2)", params![to, snapshot]).map_err(fail)?;
                Self::put_back(&tx, &previous)?;
                let revision = Self::bump(&tx)?;
                tx.commit().map_err(fail)?;
                Ok(json!({"revision": revision, "summary": match (name == "history.undo", ctx.en) {
                    (true, false) => "Đã hoàn tác",
                    (true, true) => "Undone",
                    (false, false) => "Đã làm lại",
                    (false, true) => "Redone",
                }}))
            }
            _ => Err(if ctx.en { format!("There is no action {name}.") } else { format!("Không có hành động {name}.") }),
        }
    }
}

fn text(args: &Value, key: &str) -> Result<String, String> {
    args[key].as_str().map(str::trim).filter(|s| !s.is_empty()).map(str::to_owned).ok_or_else(|| format!("Thiếu {key}."))
}

fn main() {
    let mut notes = match Notes::open() {
        Ok(notes) => notes,
        Err(error) => {
            eprintln!("notes-core: {error}");
            std::process::exit(1);
        }
    };
    let stdin = std::io::stdin();
    let mut stdout = std::io::stdout().lock();
    for line in stdin.lock().lines() {
        let Ok(line) = line else { break };
        let Ok(request) = serde_json::from_str::<Value>(&line) else { continue };
        // Notifications (no id) need no answer.
        let Some(id) = request.get("id").cloned() else { continue };
        let result = match request["method"].as_str().unwrap_or_default() {
            "initialize" => json!({
                "protocolVersion": request["params"]["protocolVersion"].as_str().unwrap_or("2025-06-18"),
                "capabilities": {"tools": {"listChanged": false}},
                "serverInfo": {"name": "notes-core", "version": env!("CARGO_PKG_VERSION")}
            }),
            "tools/list" => json!({"tools": tools()}),
            "tools/call" => {
                let name = request["params"]["name"].as_str().unwrap_or_default();
                let args = request["params"].get("arguments").cloned().unwrap_or(json!({}));
                let ctx = Ctx::of(&request["params"]["_meta"]);
                match notes.call(name, &args, &ctx) {
                    Ok(value) => json!({"content": [{"type": "text", "text": value.to_string()}], "structuredContent": value, "isError": false}),
                    Err(message) => json!({"content": [{"type": "text", "text": message}], "isError": true}),
                }
            }
            "ping" => json!({}),
            other => {
                let error = json!({"jsonrpc": "2.0", "id": id, "error": {"code": -32601, "message": format!("Unknown method {other}")}});
                let _ = writeln!(stdout, "{error}");
                let _ = stdout.flush();
                continue;
            }
        };
        let _ = writeln!(stdout, "{}", json!({"jsonrpc": "2.0", "id": id, "result": result}));
        let _ = stdout.flush();
    }
}
