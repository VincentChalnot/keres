<?php

declare(strict_types=1);

namespace App\Model;

readonly class BoardMovesData
{
    public function __construct(
        public BoardData $boardData,
        public MovesData $movesData,
    ) {
    }
}
