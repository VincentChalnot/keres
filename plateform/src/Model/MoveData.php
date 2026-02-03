<?php
declare(strict_types=1);

namespace App\Model;

readonly class MoveData
{
    private const int MOVE_DATA_SIZE = 2; // 2 bytes per move

    public function __construct(public string $data)
    {
        if (strlen($data) !== self::MOVE_DATA_SIZE) {
            throw new \InvalidArgumentException('Invalid move data size');
        }
    }
}
