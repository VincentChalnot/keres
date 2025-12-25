import {defineConfig} from "vite";
import symfonyPlugin from "vite-plugin-symfony";
import basicSsl from '@vitejs/plugin-basic-ssl';

export default defineConfig({
  plugins: [
    symfonyPlugin(),
    basicSsl(),
  ],
  server: {
    host: true,
    port: 5173,
    https: true,
    hmr: {
      host: 'localhost',
    }
  },
  build: {
    rollupOptions: {
      input: {
        app: "./assets/app.js"
      },
    }
  },
});
