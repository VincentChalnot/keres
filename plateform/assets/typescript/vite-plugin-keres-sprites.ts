// Vite plugin for Keres sprites (ESM version)
import {readFileSync, readdirSync, writeFileSync, mkdirSync, existsSync} from 'fs';
import {resolve, extname} from 'path';
import {optimize} from 'svgo';

function keresSpritesPlugin() {
    const virtualModuleId = 'virtual:keres-sprites';
    const resolvedVirtualModuleId = '\0' + virtualModuleId;

    let spriteContent = '';
    let rootDir = process.cwd(); // fallback, will be set by configResolved
    let outDir = 'dist'; // use a local variable instead of this.outDir
    let isDev = false;

    const generateSprite = () => {
        // Read the base SVG
        const boardSvgPath = resolve(rootDir, 'assets/template.svg');
        let boardSvgContent = readFileSync(boardSvgPath, 'utf-8');

        // Find <defs> section
        const defsOpenTag = '<defs>';
        const defsCloseTag = '</defs>';
        let defsStart = boardSvgContent.indexOf(defsOpenTag);
        let defsEnd = boardSvgContent.indexOf(defsCloseTag);
        let symbols = '';

        // Prepare symbols for icons and texts
        const icons = readdirSync(resolve(rootDir, 'assets/pieces/icons'), 'utf-8').filter(file => extname(file).toLowerCase() === '.svg');
        const texts = readdirSync(resolve(rootDir, 'assets/pieces/texts'), 'utf-8').filter(file => extname(file).toLowerCase() === '.svg');

        icons.forEach(file => {
            const content = readFileSync(resolve(rootDir, 'assets/pieces/icons', file), 'utf-8');
            const name = file.replace('.svg', '');
            const optimized = optimize(content, {});
            symbols += optimized.data.replace('<svg', `<symbol id="icon-${name}"`)
                .replace('</svg>', '</symbol>')
                .replace(' xmlns="http://www.w3.org/2000/svg"', '');
        });

        texts.forEach(file => {
            const content = readFileSync(resolve(rootDir, 'assets/pieces/texts', file), 'utf-8');
            const name = file.replace('.svg', '');
            const optimized = optimize(content, {});
            symbols += optimized.data.replace('<svg', `<symbol id="text-${name}"`)
                .replace('</svg>', '</symbol>')
                .replace(' xmlns="http://www.w3.org/2000/svg"', '');
        });

        // Insert symbols into <defs>
        if (defsStart !== -1 && defsEnd !== -1) {
            // Insert symbols just before </defs>
            boardSvgContent = boardSvgContent.slice(0, defsEnd) + symbols + boardSvgContent.slice(defsEnd);
        } else {
            // No <defs> section, create one after <svg ...>
            const svgOpenTagMatch = boardSvgContent.match(/<svg[^>]*>/);
            if (svgOpenTagMatch) {
                const svgOpenTagEnd = svgOpenTagMatch.index + svgOpenTagMatch[0].length;
                boardSvgContent = boardSvgContent.slice(0, svgOpenTagEnd) + '<defs>' + symbols + '</defs>' + boardSvgContent.slice(svgOpenTagEnd);
            } else {
                // Fallback: just wrap everything in a new SVG
                boardSvgContent = `<svg xmlns="http://www.w3.org/2000/svg"><defs>${symbols}</defs></svg>`;
            }
        }
        spriteContent = boardSvgContent;
    };

    return {
        name: 'keres-sprites',

        configResolved(config) {
            rootDir = config.root || process.cwd();
            outDir = config.build?.outDir || 'dist';
            isDev = config.command === 'serve';
        },

        configureServer(server) {
            // Serve the sprite dynamically in dev
            server.middlewares.use((req, res, next) => {
                if (req.url && req.url.endsWith('/build/pieces-sprite.svg')) {
                    generateSprite(); // Always up-to-date
                    res.setHeader('Content-Type', 'image/svg+xml');
                    res.end(spriteContent);
                    return;
                }
                next();
            });
        },

        buildStart() {
            generateSprite();
            // Optionally keep writing to disk for legacy reasons, but not required for dev
            if (isDev) {
                // Write the sprite to public/build for dev server (optional, not required)
                const outputDir = resolve(rootDir, 'public/build');
                if (!existsSync(outputDir)) {
                    mkdirSync(outputDir, {recursive: true});
                }
                writeFileSync(resolve(outputDir, 'pieces-sprite.svg'), spriteContent, 'utf-8');
            }
        },

        generateBundle() {
            // Write the sprite to the output directory
            this.emitFile({
                type: 'asset',
                fileName: 'pieces-sprite.svg',
                source: spriteContent
            });
        },

        resolveId(id) {
            if (id === virtualModuleId) {
                return resolvedVirtualModuleId;
            }
        },

        load(id) {
            if (id === resolvedVirtualModuleId) {
                // Return the sprite as a JS module
                return `export default ${JSON.stringify(spriteContent)}`;
            }
        },

        handleHotUpdate({file, server}) {
            // Regenerate when an SVG changes in dev
            if (file.includes('assets/pieces')) {
                generateSprite();
                // In dev, also write to disk so public/build/pieces-sprite.svg is always up-to-date
                if (isDev) {
                    const outputDir = resolve(rootDir, 'public/build');
                    if (!existsSync(outputDir)) {
                        mkdirSync(outputDir, {recursive: true});
                    }
                    writeFileSync(resolve(outputDir, 'pieces-sprite.svg'), spriteContent, 'utf-8');
                }
                server.ws.send({type: 'full-reload'});
            }
        }
    };
}

export {keresSpritesPlugin};
