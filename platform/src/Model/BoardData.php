<?php

declare(strict_types=1);

namespace App\Model;

/*
 * Helper class to parse board data binary string.
 *
 * Board data format (total 83 bytes):
 * - 81 bytes: squares (0-80), each byte represents a square with piece encoding
 * - 1 byte:
 *     bit 8: white_to_move
 *     bit 7: game_over
 *     bit 6: white_wins
 *     bit 5: draw
 *     bits 4-1: unused
 * - 1 byte: Moves without capture counter
 */

readonly class BoardData
{
    private const int BOARD_DATA_SIZE = 9 * 9 + 2; // 81 squares + 2 bytes flags
    public bool $whiteToMove;
    public bool $gameOver;
    public bool $whiteWins;
    public bool $draw;
    public int $movesWithoutCapture;

    public function __construct(public string $data)
    {
        if (self::BOARD_DATA_SIZE !== \strlen($data)) {
            throw new \InvalidArgumentException('Invalid board data size');
        }
        /** @var int $flags */
        $flags = unpack('C', $data[81])[1];
        $this->whiteToMove = (bool) ($flags & 0b10000000);
        $this->gameOver = (bool) ($flags & 0b01000000);
        $this->whiteWins = (bool) ($flags & 0b00100000);
        $this->draw = (bool) ($flags & 0b00010000);
        $this->movesWithoutCapture = \ord($data[82]);
    }

    public function getPositionData(): string
    {
        return substr($this->data, 0, 81);
    }
}
