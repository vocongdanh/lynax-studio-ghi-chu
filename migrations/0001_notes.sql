-- The notes, proposals waiting for the person, and the undo history.
CREATE TABLE notes(
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  position INTEGER NOT NULL,
  created_by TEXT NOT NULL
);
CREATE TABLE proposals(
  id TEXT PRIMARY KEY,
  note_id TEXT NOT NULL,
  title TEXT NOT NULL,
  summary TEXT NOT NULL,
  revision INTEGER NOT NULL
);
CREATE TABLE history(
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  stack TEXT NOT NULL CHECK (stack IN ('undo', 'redo')),
  snapshot TEXT NOT NULL
);
CREATE TABLE counters(key TEXT PRIMARY KEY, value INTEGER NOT NULL);
INSERT INTO counters VALUES ('revision', 0), ('next_id', 1);
