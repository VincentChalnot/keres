import {readFileSync, writeFileSync, readdirSync} from 'fs';
import {join, extname, basename} from 'path';
import {optimize} from 'svgo';
import * as cheerio from 'cheerio';

// Configuration
const INPUT_DIR = './input-svgs';
const OUTPUT_DIR = './output-svgs';
const TRANSFORM = 'translate(-44 -44)scale(.978)';

const svgoConfig = {
    multipass: true,
    plugins: [
        {name: 'convertStyleToAttrs'},
        {
            name: 'preset-default',
            params: {
                overrides: {
                    convertPathData: {
                        applyTransforms: true,
                        applyTransformsStroked: true,
                        floatPrecision: 2,
                        transformPrecision: 0,
                    }
                }
            }
        },
    ]
};


/**
 * Extracts all paths from an SVG, ignoring groups
 */
function extractPaths(svgContent) {
    const $ = cheerio.load(svgContent, {xmlMode: true});

    // Find all paths, regardless of depth, but filter out those with stroke-dasharray or fill: none in style
    const paths = [];
    $('path').each((i, elem) => {
        const style = ($(elem).attr('style') || '').replace(/\s/g, '');
        // Remove path if style contains stroke-dasharray or fill:none
        //if (/stroke-dasharray/.test(style) || /fill:none/.test(style)) {
        //    return;
        //}
        paths.push($.html(elem));
    });

    return paths;
}

/**
 * Creates a new SVG with all paths in a transformed group and sets a fixed viewBox
 */
function createTransformedSvg(paths, originalSvg) {
    const $ = cheerio.load(originalSvg, {xmlMode: true});
    const $svg = $('svg');

    // Set the new fixed viewBox and preserve xmlns
    const viewBox = '-45 -45 90 90';
    const xmlns = $svg.attr('xmlns') || 'http://www.w3.org/2000/svg';

    // Build the new SVG
    const newSvg = `<svg xmlns="${xmlns}" viewBox="${viewBox}">
  <g transform="${TRANSFORM}">
    ${paths.join('\n    ')}
  </g>
</svg>`;

    return newSvg;
}

/**
 * Processes a single SVG file
 */
function processSvgFile(inputPath, outputPath) {
    console.log(`📄 Processing: ${basename(inputPath)}`);

    try {
        // 1. Read the file
        const svgContent = readFileSync(inputPath, 'utf-8');

        // 2. Extract all paths (ignore groups)
        const paths = extractPaths(svgContent);
        console.log(`   ✓ ${paths.length} paths found`);

        // 3. Create a new SVG with a single transformed group
        const transformedSvg = createTransformedSvg(paths, svgContent);

        // 4. Optimize with SVGO (apply transform directly)
        const result = optimize(transformedSvg, svgoConfig);
        console.log(`   ✓ Optimized (${svgContent.length} → ${result.data.length} bytes)`);

        // 5. Save
        writeFileSync(outputPath, result.data, 'utf-8');
        console.log(`   ✓ Saved: ${basename(outputPath)}\n`);

        return true;
    } catch (error) {
        console.error(`   ✗ Error: ${error.message}\n`);
        return false;
    }
}

/**
 * Processes all SVG files in a directory
 */
function processDirectory() {
    console.log(`🚀 Batch SVG processing\n`);
    console.log(`📁 Input:  ${INPUT_DIR}`);
    console.log(`📁 Output: ${OUTPUT_DIR}\n`);

    const files = readdirSync(INPUT_DIR);
    const svgFiles = files.filter(file => extname(file).toLowerCase() === '.svg');

    if (svgFiles.length === 0) {
        console.log('⚠️  No SVG files found\n');
        return;
    }

    console.log(`📊 ${svgFiles.length} file(s) to process\n`);

    let success = 0;
    let failed = 0;

    svgFiles.forEach(file => {
        const inputPath = join(INPUT_DIR, file);
        const outputPath = join(OUTPUT_DIR, file);

        if (processSvgFile(inputPath, outputPath)) {
            success++;
        } else {
            failed++;
        }
    });

    console.log(`✅ Finished: ${success} succeeded, ${failed} failed`);
}

// Run the processing
processDirectory();
