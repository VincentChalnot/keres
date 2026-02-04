<?php
declare(strict_types=1);

namespace App\Service;

use App\Model\BoardMovesData;
use Symfony\Component\Mercure\HubInterface;
use Symfony\Component\Mercure\Update;

readonly class GameUpdatePublisher
{
    public function __construct(
        private HubInterface $hub,
    ) {
    }

    /**
     * Publish a game update to all clients listening to this game.
     */
    public function publishGameUpdate(string $gameUuid, BoardMovesData $boardMovesData): void
    {
        $boardData = $boardMovesData->boardData;

        // Create the update data with timestamp in microseconds
        $data = [
            'success' => true,
            'board' => base64_encode($boardData->data),
            'moves' => base64_encode($boardMovesData->movesData->toBinary()),
            'gameOver' => $boardData->gameOver,
            'whiteWins' => $boardData->whiteWins,
            'draw' => $boardData->draw,
            'timestamp' => (int) (microtime(true) * 1000000), // Microseconds since epoch
        ];

        $update = new Update(
            sprintf('game/%s', $gameUuid),
            json_encode($data),
        );

        $this->hub->publish($update);
    }
}
