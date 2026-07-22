#syntax=docker/dockerfile:1

# The slow xcaddy build (FrankenPHP recompile with custom Caddy modules) lives in
# Dockerfile.base and is published as ghcr.io/vincentchalnot/keres/php-base.
# It is only rebuilt when Dockerfile.base changes (see .github/workflows/build-base.yaml).
# This keeps the main CI build fast (~2 min instead of ~8 min).

# Base FrankenPHP image (pre-built with Infomaniak DNS module — see Dockerfile.base)
FROM ghcr.io/vincentchalnot/keres/php-base AS frankenphp_base

# Dev FrankenPHP image
FROM frankenphp_base AS frankenphp_dev

ENV APP_ENV=dev
ENV XDEBUG_MODE=off
ENV FRANKENPHP_WORKER_CONFIG=watch

RUN mv "$PHP_INI_DIR/php.ini-development" "$PHP_INI_DIR/php.ini"

RUN set -eux; \
	install-php-extensions \
		xdebug \
	;

COPY --link frankenphp/conf.d/20-app.dev.ini $PHP_INI_DIR/app.conf.d/

# Always copy the Caddyfile from source so changes take effect on every image
# build, not only when the slow base image (Dockerfile.base) is rebuilt.
COPY --link frankenphp/Caddyfile /etc/frankenphp/Caddyfile

CMD [ "frankenphp", "run", "--config", "/etc/frankenphp/Caddyfile", "--watch" ]

# Node builder stage: compile frontend assets for production
FROM node:24-alpine AS node_builder

WORKDIR /app

# Install dependencies first (layer-cached independently of source changes)
COPY --link package.json package-lock.json ./
RUN npm ci

# Copy sources needed for the build
COPY --link assets ./assets
COPY --link vite.config.js tsconfig.json* ./
RUN npm run build

# Prod FrankenPHP image
FROM frankenphp_base AS frankenphp_prod

ENV APP_ENV=prod

# Build-time-only placeholders — nothing here reaches a real service. They
# exist purely so `composer run-script post-install-cmd` (which runs
# `cache:clear`/warms the DI container) can compile without a live
# environment during image build. Docker Compose's `environment:`/`env_file`
# always override every one of these at container start (see compose.yaml /
# deploy/compose.yaml) — none of these values are ever used at runtime.
ENV APP_SECRET=build-time-placeholder \
    DATABASE_URL="postgresql://app:app@127.0.0.1:5432/app?serverVersion=16&charset=utf8" \
    SERVER_NAME=app.example.invalid \
    DEFAULT_URI=https://app.example.invalid \
    STATIC_SITE_URL=https://example.invalid \
    SESSION_COOKIE_DOMAIN=example.invalid \
    MERCURE_URL=https://app.example.invalid/.well-known/mercure \
    MERCURE_PUBLIC_URL=https://app.example.invalid/.well-known/mercure \
    MERCURE_JWT_SECRET=build-time-placeholder \
    MESSENGER_TRANSPORT_DSN="doctrine://default?auto_setup=0" \
    BACKEND_API_URL=http://backend:3000 \
    AI_BACKEND_API_URL=http://backend:3000 \
    MAILER_DSN=null://null \
    CORS_ALLOW_ORIGIN=^https://example\.invalid$ \
    OIDC_GOOGLE_CLIENT_ID= \
    OIDC_GOOGLE_CLIENT_SECRET= \
    OIDC_FACEBOOK_CLIENT_ID= \
    OIDC_FACEBOOK_CLIENT_SECRET= \
    OIDC_DISCORD_CLIENT_ID= \
    OIDC_DISCORD_CLIENT_SECRET=

RUN mv "$PHP_INI_DIR/php.ini-production" "$PHP_INI_DIR/php.ini"

COPY --link frankenphp/conf.d/20-app.prod.ini $PHP_INI_DIR/app.conf.d/

# Always copy the Caddyfile from source so changes take effect on every image
# build, not only when the slow base image (Dockerfile.base) is rebuilt.
COPY --link frankenphp/Caddyfile /etc/frankenphp/Caddyfile

# prevent the reinstallation of vendors at every changes in the source code
COPY --link composer.* symfony.* ./
RUN set -eux; \
	composer install --no-cache --prefer-dist --no-dev --no-autoloader --no-scripts --no-progress

# copy sources
COPY --link --exclude=frankenphp/ . ./
# Inject the frontend assets built by node_builder (overrides the empty public/build/)
COPY --from=node_builder /app/public/build ./public/build

RUN set -eux; \
	mkdir -p var/cache var/log var/share; \
	composer dump-autoload --classmap-authoritative --no-dev; \
	composer run-script --no-dev post-install-cmd; \
	chmod +x bin/console; sync;

# Worker image (Supervisor + Symfony Messenger)
FROM frankenphp_dev AS frankenphp_worker_dev

RUN apt-get update && apt-get install -y --no-install-recommends \
    supervisor \
    && rm -rf /var/lib/apt/lists/*

COPY --link frankenphp/supervisor/supervisord.conf /etc/supervisor/supervisord.conf
COPY --link frankenphp/supervisor/messenger-worker.conf /etc/supervisor/conf.d/messenger-worker.conf

WORKDIR /tmp

# Override le CMD et l'ENTRYPOINT de frankenphp_prod
ENTRYPOINT []
CMD ["/usr/bin/supervisord", "-c", "/etc/supervisor/supervisord.conf"]

# No HTTP server runs in this image (supervisord replaces frankenphp as PID 1),
# so the base image's curl :2019/metrics healthcheck is meaningless here.
# supervisorctl exits non-zero if any managed program (the messenger:consume
# workers) isn't RUNNING — requires the [unix_http_server]/[supervisorctl]
# RPC socket declared in supervisord.conf.
HEALTHCHECK --start-period=10s --interval=30s --timeout=5s --retries=3 \
	CMD supervisorctl status all

# Worker image (Supervisor + Symfony Messenger)
FROM frankenphp_prod AS frankenphp_worker_prod

RUN apt-get update && apt-get install -y --no-install-recommends \
    supervisor \
    && rm -rf /var/lib/apt/lists/*

COPY --link frankenphp/supervisor/supervisord.conf /etc/supervisor/supervisord.conf
COPY --link frankenphp/supervisor/messenger-worker.conf /etc/supervisor/conf.d/messenger-worker.conf

WORKDIR /tmp

# Override le CMD et l'ENTRYPOINT de frankenphp_prod
ENTRYPOINT []
CMD ["/usr/bin/supervisord", "-c", "/etc/supervisor/supervisord.conf"]

# No HTTP server runs in this image (supervisord replaces frankenphp as PID 1),
# so the base image's curl :2019/metrics healthcheck is meaningless here.
# supervisorctl exits non-zero if any managed program (the messenger:consume
# workers) isn't RUNNING — requires the [unix_http_server]/[supervisorctl]
# RPC socket declared in supervisord.conf.
HEALTHCHECK --start-period=10s --interval=30s --timeout=5s --retries=3 \
	CMD supervisorctl status all
