_default:
  @just --list

new_migration name:
  sqlx migrate add -r {{name}}

migrate:
  sqlx migrate run

migrate_revert:
  sqlx migrate revert