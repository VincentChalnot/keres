<?php

declare(strict_types=1);

namespace App\Event;

use Symfony\Contracts\EventDispatcher\Event;

class GameUpdateEvent extends Event
{
    public function __construct(
        private readonly string $gameUuid,
    ) {
    }

    public function getGameUuid(): string
    {
        return $this->gameUuid;
    }
}
