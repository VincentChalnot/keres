import {defineConfig} from "vite";
import symfonyPlugin from "vite-plugin-symfony";
import {keresSpritesPlugin} from './assets/typescript/vite-plugin-keres-sprites';

export default defineConfig({
    plugins: [
        symfonyPlugin(),
        keresSpritesPlugin(),
    ],
    server: {
        host: '0.0.0.0',                                  // accept traffic from Traefik
        port: 5173,
        strictPort: true,                                 // fail fast if 5173 is busy
        // Requests reach this server two ways: directly from the browser
        // via Traefik (Host: vite.app.$SERVER_NAME), and internally from
        // the php container's own-origin asset proxy (pentatrion/vite
        // -bundle's ViteController::proxyBuild, Host: node:5173) for any
        // `/build/*` asset requested relative to the app's own origin.
        // Both are already trusted infra (firewalled compose network +
        // Traefik's own Host() routing rule), so skip vite's Host-header
        // allowlist rather than trying to enumerate every internal alias.
        allowedHosts: true,
        // SERVER_NAME is the same bare dev domain compose.yaml derives every
        // other URL from (see .env.example) — keeps the domain defined once
        // instead of hardcoding it again here.
        origin: `https://vite.app.${process.env.SERVER_NAME || 'local.playkeres.com'}`,  // absolute URLs emitted for HMR client
        cors: {
            origin: `https://app.${process.env.SERVER_NAME || 'local.playkeres.com'}`,
            credentials: true,
        },
        hmr: {
            // No `host`/`port` here: those also control what the HMR
            // websocket server *binds* to (see vite's ws.ts - when
            // hmr.port differs from server.port, vite spins up a second,
            // standalone server and calls `listen(port, host)` on it,
            // literally DNS-resolving `host` as a bind address). Since
            // Traefik already proxies 443 -> 5173, the HMR socket must
            // stay on the *same* server as the main dev server (0.0.0.0:5173).
            // `clientPort` only affects the URL embedded in the browser
            // client, so that's the only override we need.
            clientPort: 443,
            protocol: 'wss',
        },
    },
    build: {
        outDir: 'public/build',
        rollupOptions: {
            input: {
                app: "./assets/app.js",
                play: "./assets/typescript/src/app.ts"
            },
        },
    },
});
