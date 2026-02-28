<?php

declare(strict_types=1);

namespace App\Service;

use Symfony\Component\DependencyInjection\Attribute\Autowire;

/**
 * Generates standalone SVG images from binary board data (83 bytes).
 *
 * The rendering logic mirrors the TypeScript SVGBoardView: same board dimensions,
 * piece placement, coordinate labels, stacking offset, and color/rotation classes.
 */
class BoardRenderer
{
    private const int BOARD_SIZE = 9;
    private const int SQUARE_WIDTH = 100;
    private const int SQUARE_HEIGHT = 80;
    private const int STACKED_OFFSET = 23;
    private const int COORD_WIDTH = 25;
    private const int COORD_HEIGHT = 25;

    private const array PIECE_CODES = [
        0b001 => 'soldier',
        0b010 => 'bishop',
        0b011 => 'rook',
        0b100 => 'paladin',
        0b101 => 'guard',
        0b110 => 'knight',
        0b111 => 'ballista',
    ];

    private const array TILE_COLORS = ['#d2b48c', '#f5f5dc'];
    private const string TILE_STROKE = '#55442d';

    private ?string $defsContent = null;

    public function __construct(
        #[Autowire('%kernel.project_dir%')]
        private readonly string $projectDir,
    ) {
    }

    /**
     * Render a standalone SVG from 83-byte binary board data.
     */
    public function renderSvg(string $boardData, bool $flipped = false): string
    {
        if (\strlen($boardData) !== 83) {
            throw new \InvalidArgumentException('Board data must be exactly 83 bytes');
        }

        $boardWidth = self::BOARD_SIZE * self::SQUARE_WIDTH;
        $boardHeight = self::BOARD_SIZE * self::SQUARE_HEIGHT;

        $svg = '<?xml version="1.0" encoding="UTF-8"?>' . "\n";
        $svg .= sprintf(
            '<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="%d 0 %d %d">',
            -self::COORD_WIDTH,
            $boardWidth + self::COORD_WIDTH,
            $boardHeight + self::COORD_HEIGHT,
        );
        $svg .= "\n";

        $svg .= '<style>' . "\n" . $this->getCss() . "\n" . '</style>' . "\n";
        $svg .= $this->getDefsContent() . "\n";

        $svg .= '<g id="board-layer">' . "\n" . $this->renderBoard() . '</g>' . "\n";
        $svg .= '<g id="coords-layer">' . "\n" . $this->renderCoordinates($flipped) . '</g>' . "\n";
        $svg .= '<g id="pieces-layer">' . "\n" . $this->renderPieces($boardData, $flipped) . '</g>' . "\n";

        $svg .= '</svg>';

        return $svg;
    }

    /**
     * Convert an SVG string to a raster format using rsvg-convert.
     *
     * @param string $svg    The SVG content
     * @param string $format One of 'png', 'jpg', 'webp'
     *
     * @return string The raster image binary content
     */
    public function convertToRaster(string $svg, string $format): string
    {
        $formatMap = [
            'png' => 'png',
            'jpg' => 'jpeg',
            'webp' => 'webp',
        ];

        $rsvgFormat = $formatMap[$format] ?? throw new \InvalidArgumentException(sprintf('Unsupported format: %s', $format));

        $descriptors = [
            0 => ['pipe', 'r'],
            1 => ['pipe', 'w'],
            2 => ['pipe', 'w'],
        ];

        $process = proc_open(
            ['rsvg-convert', '--format', $rsvgFormat, '--width', '925'],
            $descriptors,
            $pipes,
        );

        if (!\is_resource($process)) {
            throw new \RuntimeException('Failed to start rsvg-convert. Please install librsvg2-bin.');
        }

        fwrite($pipes[0], $svg);
        fclose($pipes[0]);

        $output = stream_get_contents($pipes[1]);
        fclose($pipes[1]);

        $error = stream_get_contents($pipes[2]);
        fclose($pipes[2]);

        $exitCode = proc_close($process);

        if ($exitCode !== 0) {
            throw new \RuntimeException(sprintf('rsvg-convert failed (exit %d): %s', $exitCode, $error));
        }

        return $output;
    }

    private function getCss(): string
    {
        return <<<'CSS'
.p-w { --piece-bg: #fff; --piece-fg: #000; }
.p-b { --piece-bg: #000; --piece-fg: #fff; }
.p-r { --icon-rotation: 180deg; }
.piece-icon { transform-origin: 45px 35px; transform: rotate(var(--icon-rotation, 0deg)); }
.coord-label { font-size: 16px; font-family: sans-serif; fill: #55442d; pointer-events: none; }
CSS;
    }

    private function getDefsContent(): string
    {
        if ($this->defsContent !== null) {
            return $this->defsContent;
        }

        $templatePath = $this->projectDir . '/assets/template.svg';
        $templateContent = file_get_contents($templatePath);

        $symbols = '';

        // Read icon SVGs and convert to <symbol> elements
        $iconsDir = $this->projectDir . '/assets/pieces/icons';
        foreach (glob($iconsDir . '/*.svg') as $file) {
            $name = basename($file, '.svg');
            $content = file_get_contents($file);
            $content = preg_replace('/<svg\b/', '<symbol id="icon-' . $name . '"', $content);
            $content = str_replace('</svg>', '</symbol>', $content);
            $content = str_replace(' xmlns="http://www.w3.org/2000/svg"', '', $content);
            $symbols .= $content . "\n";
        }

        // Read text SVGs and convert to <symbol> elements
        $textsDir = $this->projectDir . '/assets/pieces/texts';
        foreach (glob($textsDir . '/*.svg') as $file) {
            $name = basename($file, '.svg');
            $content = file_get_contents($file);
            $content = preg_replace('/<svg\b/', '<symbol id="text-' . $name . '"', $content);
            $content = str_replace('</svg>', '</symbol>', $content);
            $content = str_replace(' xmlns="http://www.w3.org/2000/svg"', '', $content);
            $symbols .= $content . "\n";
        }

        // Insert symbols before </defs> in the template
        $defsEnd = strpos($templateContent, '</defs>');
        if ($defsEnd !== false) {
            $templateContent = substr($templateContent, 0, $defsEnd) . $symbols . substr($templateContent, $defsEnd);
        }

        // Extract the <defs>...</defs> block
        if (preg_match('/<defs>.*<\/defs>/s', $templateContent, $matches)) {
            $this->defsContent = $matches[0];
        } else {
            $this->defsContent = '<defs>' . $symbols . '</defs>';
        }

        return $this->defsContent;
    }

    private function renderBoard(): string
    {
        $result = '';
        for ($row = 0; $row < self::BOARD_SIZE; $row++) {
            for ($col = 0; $col < self::BOARD_SIZE; $col++) {
                $fill = self::TILE_COLORS[($row + $col) % 2];
                $result .= sprintf(
                    '<rect x="%d" y="%d" width="%d" height="%d" fill="%s" stroke="%s" stroke-width="1"/>',
                    $col * self::SQUARE_WIDTH,
                    $row * self::SQUARE_HEIGHT,
                    self::SQUARE_WIDTH,
                    self::SQUARE_HEIGHT,
                    $fill,
                    self::TILE_STROKE,
                ) . "\n";
            }
        }

        return $result;
    }

    private function renderCoordinates(bool $flipped): string
    {
        $result = '';
        $columns = $flipped
            ? ['I', 'H', 'G', 'F', 'E', 'D', 'C', 'B', 'A']
            : ['A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I'];

        $boardHeight = self::BOARD_SIZE * self::SQUARE_HEIGHT;

        for ($row = 0; $row < self::BOARD_SIZE; $row++) {
            $rowNumber = $flipped ? ($row + 1) : (self::BOARD_SIZE - $row);
            $result .= sprintf(
                '<text class="coord-label" x="%.1f" y="%.1f" text-anchor="middle" dominant-baseline="central">%d</text>',
                -(self::COORD_WIDTH / 2),
                $row * self::SQUARE_HEIGHT + self::SQUARE_HEIGHT / 2,
                $rowNumber,
            ) . "\n";
        }

        for ($col = 0; $col < self::BOARD_SIZE; $col++) {
            $result .= sprintf(
                '<text class="coord-label" x="%.1f" y="%.1f" text-anchor="middle" dominant-baseline="central">%s</text>',
                $col * self::SQUARE_WIDTH + self::SQUARE_WIDTH / 2,
                $boardHeight + self::COORD_HEIGHT / 2,
                $columns[$col],
            ) . "\n";
        }

        return $result;
    }

    private function renderPieces(string $boardData, bool $flipped): string
    {
        $result = '';

        for ($i = 0; $i < 81; $i++) {
            $byte = \ord($boardData[$i]);
            if ($byte === 0) {
                continue;
            }

            $piece = $this->decodePiece($byte);
            if ($piece === null) {
                continue;
            }

            $actualIndex = $flipped ? (80 - $i) : $i;
            $col = $actualIndex % self::BOARD_SIZE;
            $row = intdiv($actualIndex, self::BOARD_SIZE);
            $x = $col * self::SQUARE_WIDTH;
            $y = $row * self::SQUARE_HEIGHT;

            $colorClass = $piece['color'] ? 'p-w' : 'p-b';
            $reversed = ($piece['color'] xor $flipped) ? '' : ' p-r';
            $classes = 'piece ' . $colorClass . $reversed;

            $result .= sprintf(
                '<use href="#piece-%s" class="%s" x="%d" y="%d"/>',
                $piece['bottom'],
                $classes,
                $x,
                $y,
            ) . "\n";

            if ($piece['top'] !== null) {
                $result .= sprintf(
                    '<use href="#piece-%s" class="%s" x="%d" y="%d"/>',
                    $piece['top'],
                    $classes,
                    $x,
                    $y - self::STACKED_OFFSET,
                ) . "\n";
            }
        }

        return $result;
    }

    /**
     * Decode a piece byte into its components.
     *
     * @return array{color: bool, bottom: string, top: string|null}|null
     */
    private function decodePiece(int $byte): ?array
    {
        if ($byte === 0) {
            return null;
        }

        $color = (bool) (($byte >> 6) & 0b1);
        $payload = $byte & 0b00111111;

        // King: special encoding
        if ($payload === 0b111000) {
            return ['color' => $color, 'bottom' => 'king', 'top' => null];
        }

        $topCode = ($payload >> 3) & 0b111;
        $bottomCode = $payload & 0b111;

        if ($topCode === 0) {
            // Single piece
            if (isset(self::PIECE_CODES[$bottomCode])) {
                return ['color' => $color, 'bottom' => self::PIECE_CODES[$bottomCode], 'top' => null];
            }
        } else {
            // Stacked piece
            if (isset(self::PIECE_CODES[$topCode], self::PIECE_CODES[$bottomCode])) {
                return ['color' => $color, 'bottom' => self::PIECE_CODES[$bottomCode], 'top' => self::PIECE_CODES[$topCode]];
            }
        }

        return null;
    }
}
