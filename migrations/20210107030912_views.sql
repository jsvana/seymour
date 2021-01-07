CREATE TABLE IF NOT EXISTS views (
  user_id INT NOT NULL,
  feed_entry_id INT NOT NULL,

  FOREIGN KEY(user_id) REFERENCES users(id),
  FOREIGN KEY(feed_entry_id) REFERENCES feed_entries(id)
);
