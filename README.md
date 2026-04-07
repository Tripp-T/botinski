# botinski

A modular, multi-platform bot framework written in Rust. Currently supports Discord, with a built-in HTTP server for health checking and OAuth/SSO groundwork.

## Features

- **Discord bot** via [poise](https://github.com/serenity-rs/poise) — slash commands and prefix commands
- **HTTP server** via [axum](https://github.com/tokio-rs/axum) — health check endpoint, extensible API router
- **SQLite database** via [sqlx](https://github.com/launchbait/sqlx) with built-in migration support
- **Living config** — JSON or TOML config file, persisted atomically on shutdown
- **Role & UID-based admin system** — configurable per-guild admin roles and user IDs
- **Graceful shutdown** — Ctrl+C and SIGTERM both cleanly drain all subsystems
- **Nix flake** — reproducible dev shell and Docker image builds

---

## Prerequisites

- Rust (stable) — or use the Nix dev shell (recommended)
- A Discord application & bot token — create one at the [Discord Developer Portal](https://discord.com/developers/applications)
- `sqlx-cli` — for managing database migrations
- `just` — optional, for migration shortcuts

### With Nix (recommended)

```sh
nix develop        # enters the dev shell with all tools
direnv allow       # if using direnv / .envrc
```

### Without Nix

```sh
cargo install sqlx-cli
cargo install just
```

---

## Configuration

botinski is configured via **environment variables** (or CLI flags) and a **config file**.

### Environment Variables

| Variable | Flag | Required | Default | Description |
| --- | --- | --- | --- | --- |
| `CONFIG_PATH` | `-c` | ✅ | — | Path to the JSON or TOML config file |
| `DATABASE_URL` | `--database-url` | ✅ | — | SQLite connection string (e.g. `sqlite://botinski.db`) |
| `DISCORD_TOKEN` | `--discord-token` | ✅ | — | Bot token from the Discord Developer Portal |
| `DISCORD_CLIENT_ID` | `--discord-client-id` | ✅ | — | OAuth2 Client ID |
| `DISCORD_CLIENT_SECRET` | `--discord-client-secret` | ✅ | — | OAuth2 Client Secret |
| `HTTP_ADDR` | `--http-addr` | ❌ | `127.0.0.1:3000` | Address and port for the HTTP server |
| `DISCORD_SKIP_REGISTER_COMMANDS` | `--discord-skip-register-commands` | ❌ | `false` | Skip global slash command registration on startup |
| `RUST_LOG` | — | ❌ | — | Log level filter (e.g. `botinski=debug`, `info`) |

### Config File

If the config file does not exist, a default one will be written automatically on first run. Supported formats: `.json`, `.toml`.

**Example `config.toml`:**

```toml
[discord]
command_prefix = "\\"

# Optional: restrict admin commands to specific Discord user IDs
# admin_uids = ["123456789012345678"]

# Optional: restrict admin commands to members with specific role IDs
# admin_roles = ["987654321098765432"]
```

---

## Database Setup

botinski uses SQLite via `sqlx` with versioned migrations stored in `migrations/`.

```sh
# Create the database and run all migrations
export DATABASE_URL="sqlite://botinski.db"
sqlx database create
sqlx migrate run

# Or use just:
just migrate
```

To create a new migration:

```sh
just new_migration <name>
# e.g.: just new_migration add_guild_settings
```

To revert the last migration:

```sh
just migrate_revert
```

---

## Building with Nix

```sh
# Build the binary
nix build

# Build a Docker image
nix build .#docker
docker load < result
docker run --env-file .env botinski:latest
```
