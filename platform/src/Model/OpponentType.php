<?php

declare(strict_types=1);

namespace App\Model;

enum OpponentType: int
{
    case AI = 0;
    case HOTSEAT = 1;
}
