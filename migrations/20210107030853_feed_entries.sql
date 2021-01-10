CREATE TABLE IF NOT EXISTS feed_entries (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  feed_id INTEGER NOT NULL,
  title TEXT NOT NULL,
  published_at TEXT NOT NULL,
  url TEXT NOT NULL,

  FOREIGN KEY(feed_id) REFERENCES feeds(id) ON DELETE CASCADE,
  UNIQUE(feed_id, published_at, url)
);
