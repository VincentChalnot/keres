# Running the Messenger Worker for AI Moves

## Overview
The AI move processing uses Symfony Messenger with a database transport. Messages need to be consumed by a worker process.

## Starting the Worker

### Development
```bash
php bin/console messenger:consume async -vv
```

The `-vv` flag provides verbose output for debugging.

### Production
For production, you should use a process supervisor like Supervisor or systemd to keep the worker running.

#### Using Supervisor
Create `/etc/supervisor/conf.d/keres-messenger.conf`:
```ini
[program:keres-messenger]
command=php /path/to/plateform/bin/console messenger:consume async --time-limit=3600
user=www-data
numprocs=2
startsecs=0
autostart=true
autorestart=true
process_name=%(program_name)s_%(process_num)02d
```

Then reload supervisor:
```bash
sudo supervisorctl reread
sudo supervisorctl update
sudo supervisorctl start keres-messenger:*
```

## Docker Integration

### Adding to compose.yaml
Add a messenger worker service to your compose.yaml:

```yaml
  messenger-worker:
    image: ${IMAGES_PREFIX:-}app-php
    restart: unless-stopped
    environment:
      DATABASE_URL: postgresql://${POSTGRES_USER:-app}:${POSTGRES_PASSWORD:-!ChangeMe!}@database:5432/${POSTGRES_DB:-app}?serverVersion=${POSTGRES_VERSION:-15}&charset=${POSTGRES_CHARSET:-utf8}
    command: php bin/console messenger:consume async --time-limit=3600
    depends_on:
      - database
    volumes:
      - caddy_data:/data
      - caddy_config:/config
```

This will automatically start a worker that:
- Consumes messages from the async transport
- Restarts every hour (--time-limit=3600)
- Automatically restarts if it crashes

## Monitoring

### Check Worker Status
```bash
# View messenger stats
php bin/console messenger:stats

# View failed messages
php bin/console messenger:failed:show
```

### Retry Failed Messages
```bash
# Retry all failed messages
php bin/console messenger:failed:retry

# Retry specific message
php bin/console messenger:failed:retry [id]
```

## Development Without Worker

If you're developing and don't want to run a separate worker process, you can use the sync transport temporarily:

In `config/packages/messenger.yaml`:
```yaml
routing:
    'App\Message\ProcessAiMoveMessage': sync
```

This will process messages synchronously (blocking), which is fine for development but not recommended for production.

## Troubleshooting

### Messages Not Being Processed
1. Check if worker is running: `ps aux | grep messenger:consume`
2. Check database for pending messages: `SELECT * FROM messenger_messages WHERE delivered_at IS NULL;`
3. Check logs: `tail -f var/log/prod.log` or `docker-compose logs -f php`

### Database Connection Issues
Make sure the worker has access to the database with the correct credentials from `.env` or environment variables.

### Memory Issues
If the worker consumes too much memory over time, adjust the `--time-limit` or add `--memory-limit`:
```bash
php bin/console messenger:consume async --time-limit=3600 --memory-limit=256M
```
