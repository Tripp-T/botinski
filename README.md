# botinski

A modular, multi-platform bot framework written in Rust. Currently a work in progress.

## Prerequisites

- Rust (stable) — or use the Nix dev shell (recommended)
- A Discord application & bot token — create one at the [Discord Developer Portal](https://discord.com/developers/applications)
- `sqlx-cli` — for managing database migrations
- `just` — optional, for migration shortcuts

### Getting started

This project uses git hooks to keep .sqlx metadata current for queries.
Run the following command to have this hook run on commit.

```sh
git config core.hooksPath .githooks
```

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
# build the binary
nix build

# or, build a Docker image
nix build .#docker
docker load < result
docker run --env-file .env botinski:latest
```
