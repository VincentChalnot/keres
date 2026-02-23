<?php
declare(strict_types=1);

namespace App\MessageHandler;

use App\Engine\GameEngine;
use App\Message\ProcessAiMoveMessage;
use App\Repository\GameRepository;
use App\Service\GameUpdatePublisher;
use Symfony\Component\Messenger\Attribute\AsMessageHandler;
use Symfony\Component\Uid\Uuid;

#[AsMessageHandler]
readonly class ProcessAiMoveHandler
{
    public function __construct(
        private GameRepository $gameRepository,
        private GameEngine $gameEngine,
        private GameUpdatePublisher $gameUpdatePublisher,
    ) {
    }

    public function __invoke(ProcessAiMoveMessage $message): void
    {
        $game = $this->gameRepository->findByUuid(Uuid::fromString($message->gameUuid));
        if (!$game) {
            throw new \RuntimeException('Game not found: '.$message->gameUuid);
        }

        $boardMovesData = $this->gameEngine->aiMove($game);

        // Forward to PublishMoveMessage to update game state and notify clients
        $this->gameUpdatePublisher->publishGameUpdate($message->gameUuid, $boardMovesData);
    }
}
