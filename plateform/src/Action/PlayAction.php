<?php
declare(strict_types=1);

namespace App\Action;

use Symfony\Component\HttpKernel\Attribute\AsController;
use Symfony\Component\Routing\Attribute\Route;

#[AsController]
class PlayAction
{
    #[Route(
        path: '/play/{uid}',
        name: 'play',
    )]
    public function __(string $uid): array
    {
        // @todo fetch game by $uid and return game data
        return [
            'moves' => 'PBmGA08eB0RFHghNvBQBBikLBAcWXZcPlgWMCQ==',
        ];
    }
}
