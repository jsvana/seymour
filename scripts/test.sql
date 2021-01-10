DELETE FROM feeds;
DELETE FROM users;
DELETE FROM feed_entries;
DELETE FROM views;
DELETE FROM subscriptions;

INSERT INTO feeds
  (url)
VALUES
  ("gemini://gemini.conman.org/boston.gemini"),
  ("gemini://skyjake.fi:1965/gemlog/");

INSERT INTO users
  (username)
VALUES
  ("jsvana"),
  ("ichbinjoe");

INSERT INTO subscriptions
  (user_id, feed_id)
VALUES
  (1, 2);
