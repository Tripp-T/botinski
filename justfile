_default:
  @just --list

dev:
  cargo watch -x 'run --features dev' --ignore data

clippy:
  cargo watch -x 'clippy' --ignore data

new_migration name:
  sqlx migrate add -r {{name}}

migrate:
  sqlx migrate run

migrate_revert:
  sqlx migrate revert

commit:
  cargo sqlx prepare
  git add .sqlx
  git commit