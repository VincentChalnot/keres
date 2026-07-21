# Production deployment

This directory contains everything needed to run the **Keres app** in
production. The static site (`playkeres.com`) is served separately by
Cloudflare Pages from `../website/` — see the Cloudflare dashboard for that
project, not this directory.

## Architecture (prod)

```
Internet ──▶ Cloudflare (TLS, DDoS, cache)
                │
                ├─▶ playkeres.com        → Cloudflare Pages (Hugo build)
                │
                └─▶ app.playkeres.com    → Traefik (TLS) → this compose
                                                       │
                                                       ├─▶ php (FrankenPHP/Symfony)
                                                       ├─▶ php-worker (Messenger)
                                                       ├─▶ backend (Rust engine)
                                                       └─▶ database (Postgres)
```

`Traefik` is **out of scope** of this repo — it is run as a separate process
on the prod host (typically via the standard `traefik:v3` image with a
companion compose). Its responsibilities:

- Terminate TLS for `app.playkeres.com` (Infomaniak DNS-01 ACME → LetsEncrypt)
- Forward cleartext HTTP to the `php` service on the shared `proxy` network
- Inject `X-Forwarded-For` / `X-Forwarded-Proto` so Symfony can recover the
  real client IP and trust the upstream scheme

## Files in this directory

| File              | Purpose                                                     |
|-------------------|-------------------------------------------------------------|
| `compose.yaml`    | Production app stack (php, php-worker, backend, database).  |
| `db-backup.sh`    | Daily Postgres backup via `docker exec pg_dump` + cron job. |
| `.env.example`    | Template for the production `.env` — copy to `.env`.        |

## Initial deploy

```bash
# 1. Copy this directory to the prod server (rsync, scp, whatever).
#    Everything in this directory lands at e.g. /opt/keres/deploy/.

# 2. Create the env file:
cp .env.example .env
$EDITOR .env       # fill in APP_SECRET, POSTGRES_PASSWORD, MERCURE_JWT_SECRET

# 3. Ensure the external Traefik `proxy` network exists:
docker network create proxy 2>/dev/null || true

# 4. Pull images and start:
docker compose pull
docker compose up -d

# 5. Run migrations (one-off):
docker compose exec php bin/console doctrine:migrations:migrate --no-interaction
```

## Deploying a new image tag

CI builds and pushes `ghcr.io/vincentchalnot/keres/php:<sha>` and
`…/php-worker:<sha>` / `…/backend:<sha>` for each commit on `main`. To deploy
a specific build:

```bash
IMAGES_TAG=sha-abc123 docker compose pull
IMAGES_TAG=sha-abc123 docker compose up -d
```

`IMAGES_TAG` defaults to `latest` if unset.

## Backups

`db-backup.sh` produces a `pg_dump` of the running Postgres container,
compressed, with a 7-day retention. Recommended crontab entry:

```cron
# /etc/cron.d/keres-backup
15 3 * * * root /opt/keres/deploy/db-backup.sh >> /var/log/keres-backup.log 2>&1
```

The script writes to `${BACKUP_DIR:-/var/backups/keres}` by default — override
the `BACKUP_DIR` env var if you prefer another location.

To restore:

```bash
# Find the dump you want:
ls -lh /var/backups/keres/

# Restore (this DROPS and recreates the target DB):
gunzip -c /var/backups/keres/keres-YYYYMMDD-HHMMSS.sql.gz | \
  docker compose exec -T database psql -U "$POSTGRES_USER" -d "$POSTGRES_DB"
```

See the bottom of `db-backup.sh` for the full restore procedure.

## Logs

```bash
docker compose logs -f php           # Symfony / Caddy
docker compose logs -f php-worker    # Messenger consumer
docker compose logs -f backend       # Rust engine
```

## Updating

```bash
git pull                       # in the repo checkout
cd deploy
IMAGES_TAG=<new-sha> docker compose pull
IMAGES_TAG=<new-sha> docker compose up -d
docker compose exec php bin/console cache:clear --env=prod
```

## What's NOT here

- **Traefik config** — runs in a sibling compose, see your host's setup.
- **Cloudflare Pages** — separate project, builds Hugo from `../website/`.
- **OIDC provider consoles** — `app.playkeres.com/auth/callback` must be
  registered on each provider (Google Cloud Console, Discord Developer
  Portal, Facebook for Developers).
