<?php
declare(strict_types=1);

namespace App\MessageHandler;

use App\Engine\EngineApi;
use App\Message\PublishMoveMessage;
use App\Model\BoardMovesData;
use App\Repository\GameRepository;
use App\Service\GameUpdatePublisher;
use Symfony\Component\Messenger\Attribute\AsMessageHandler;
use Symfony\Component\Uid\Uuid;

#[AsMessageHandler]
readonly class PublishMoveHandler
{
    public function __construct(
        private GameRepository $gameRepository,
        private EngineApi $engineApi,
        private GameUpdatePublisher $gameUpdatePublisher,
    ) {
    }

    public function __invoke(PublishMoveMessage $message): void
    {
        $game = $this->gameRepository->findByUuid(Uuid::fromString($message->gameUuid));
        if (!$game) {
            throw new \RuntimeException('Game not found: '.$message->gameUuid);
        }

        $movesData = $game->getMovesData();
        $boardData = $this->engineApi->replayMoves($movesData);

        $boardMovesData = new BoardMovesData($boardData, $movesData);

        // Publish update to Mercure
        $this->gameUpdatePublisher->publishGameUpdate($message->gameUuid, $boardMovesData);
    }
}
