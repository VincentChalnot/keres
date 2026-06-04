import {readFileSync, writeFileSync, readdirSync} from 'fs';
import {join, extname, basename} from 'path';
import {optimize} from 'svgo';

// Configuration
const INPUT_DIR = './pieces/texts';
const OUTPUT_DIR = './pieces/texts/optimized';

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
 * Processes a single SVG file
 */
function processSvgFile(inputPath, outputPath) {
    console.log(`📄 Processing: ${basename(inputPath)}`);

    try {
        // 1. Read the file
        const svgContent = readFileSync(inputPath, 'utf-8');

        // 4. Optimize with SVGO (apply transform directly)
        const result = optimize(svgContent, svgoConfig);
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
