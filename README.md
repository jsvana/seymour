# Seymour

`seymour` is a headless feed aggregator for the [gemini](https://gemini.circumlunar.space) protocol.

## Configuration

You'll need to use a client to connect to `seymour`. Technically you can do so with netcat.

## Installation

```
cargo install sqlx-cli
sqlx database create
sqlx migrate run
```

### Debian

```
sudo apt install libsqlite3-dev sqlite3 libssl-dev
```

## License

[MIT](LICENSE.md)
