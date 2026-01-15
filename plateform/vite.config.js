import {defineConfig} from "vite";
import symfonyPlugin from "vite-plugin-symfony";
import fs from 'fs';

export default defineConfig({
  plugins: [
    symfonyPlugin(),
  ],
  server: {
    host: 'local.playkeres.com',
    port: 5173,
    https: {
      key: fs.readFileSync('/app/frankenphp/certs/privkey.pem'),
      cert: fs.readFileSync('/app/frankenphp/certs/fullchain.pem'),
    },
    hmr: {
      host: 'local.playkeres.com',
    },
    cors: {
      origin: 'https://local.playkeres.com',
      credentials: true,
    },
  },
  build: {
    rollupOptions: {
      input: {
        app: "./assets/app.js",
        play: "./assets/typescript/src/app.ts"
      },
    }
  },
});
