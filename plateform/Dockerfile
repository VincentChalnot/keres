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

RUN mv "$PHP_INI_DIR/php.ini-production" "$PHP_INI_DIR/php.ini"

COPY --link frankenphp/conf.d/20-app.prod.ini $PHP_INI_DIR/app.conf.d/

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
	composer dump-env prod; \
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
