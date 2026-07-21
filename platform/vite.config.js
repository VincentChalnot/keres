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
        origin: 'https://vite.app.local.playkeres.com',  // absolute URLs emitted for HMR client
        cors: {
            origin: 'https://app.local.playkeres.com',
            credentials: true,
        },
        hmr: {
            host: 'vite.app.local.playkeres.com',
            port: 443,                                    // Traefik's TLS port
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
