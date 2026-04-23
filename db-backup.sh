#!/usr/bin/env bash
# db-backup.sh — Hot-safe PostgreSQL backup via pg_dump (runs inside the container).
#
# Usage: ./db-backup.sh
#
# Place this script next to prod_compose.yaml.
# POSTGRES_USER, POSTGRES_DB and PGPASSWORD are read directly from the running
# database container's environment — no need to source the .env file here.
#
# Recommended crontab (daily at 02:00, keep 30 days):
#   0 2 * * * /path/to/db-backup.sh >> /path/to/backups/backup.log 2>&1
#
# Backups are written to a `backups/` folder next to this script.
# Files older than RETENTION_DAYS are pruned automatically.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
BACKUP_DIR="${SCRIPT_DIR}/backups"
RETENTION_DAYS="${RETENTION_DAYS:-30}"

COMPOSE="docker compose --project-directory ${SCRIPT_DIR} --file ${SCRIPT_DIR}/prod_compose.yaml"

# Read connection parameters straight from the container environment.
POSTGRES_USER="$($COMPOSE exec -T database printenv POSTGRES_USER)"
POSTGRES_DB="$($COMPOSE exec -T database printenv POSTGRES_DB)"

TIMESTAMP="$(date +%Y%m%d_%H%M%S)"
BACKUP_FILE="${BACKUP_DIR}/${TIMESTAMP}_${POSTGRES_DB}.sql.gz"

mkdir -p "${BACKUP_DIR}"

echo "[$(date -Iseconds)] Starting backup of database '${POSTGRES_DB}' → ${BACKUP_FILE}"

# pg_dump runs inside the database container so there is no network/port exposure.
# --format=plain | gzip produces a plain-SQL gz that is trivial to restore with psql.
$COMPOSE exec -T database \
  pg_dump \
    --username="${POSTGRES_USER}" \
    --dbname="${POSTGRES_DB}" \
  | gzip -9 > "${BACKUP_FILE}"

echo "[$(date -Iseconds)] Backup complete: ${BACKUP_FILE} ($(du -sh "${BACKUP_FILE}" | cut -f1))"

# Prune old backups
echo "[$(date -Iseconds)] Pruning backups older than ${RETENTION_DAYS} days…"
find "${BACKUP_DIR}" -maxdepth 1 -name "*.sql.gz" -mtime "+${RETENTION_DAYS}" -delete
echo "[$(date -Iseconds)] Done."

# ── Restore instructions ──────────────────────────────────────────────────────
# To restore a backup:
#   gunzip -c backups/<timestamp>_<db>.sql.gz | \
#     docker compose --file prod_compose.yaml exec -T database \
#       psql --username=<POSTGRES_USER> --dbname=<POSTGRES_DB>
