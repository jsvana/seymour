CREATE TABLE IF NOT EXISTS subscriptions (
  user_id INT NOT NULL,
  feed_id INT NOT NULL,

  FOREIGN KEY(user_id) REFERENCES users(id),
  FOREIGN KEY(feed_id) REFERENCES feeds(id)
);
