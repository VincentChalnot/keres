import {defineConfig} from "vite";
import symfonyPlugin from "vite-plugin-symfony";
import {keresSpritesPlugin} from './assets/typescript/vite-plugin-keres-sprites';
import fs from 'fs';

export default defineConfig({
    plugins: [
        symfonyPlugin(),
        keresSpritesPlugin(),
    ],
    server: {
        host: 'local.playkeres.com',
        port: 5173,
        https: fs.existsSync('/app/frankenphp/certs/privkey.pem') ? {
            key: fs.readFileSync('/app/frankenphp/certs/privkey.pem'),
            cert: fs.readFileSync('/app/frankenphp/certs/fullchain.pem'),
        } : undefined,
        hmr: {
            host: 'local.playkeres.com',
        },
        cors: {
            origin: 'https://local.playkeres.com',
            credentials: true,
        },
    },
    build: {
        outDir: 'public/build',
        rollupOptions: {
            input: {
                app: "./assets/app.js",
                play: "./assets/typescript/src/app.ts"
            },
        }
    },
});
