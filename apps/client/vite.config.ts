import {defineConfig} from 'vite';

export default defineConfig({
    root: '.',
    publicDir: './public',
    build: {
        outDir: 'dist',
        emptyOutDir: true,
        sourcemap: true,
        rollupOptions: {
            input: {
                main: './index.html'
            }
        }
    },
    server: {
        port: 8080,
        host: '0.0.0.0',
        allowedHosts: ['bastion'],
        strictPort: true
    }
});
