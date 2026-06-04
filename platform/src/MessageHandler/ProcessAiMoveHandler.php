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

        if ($game->getGameMoves()->count() !== $message->moveCounter) {
            // Move has already been played, just re-publish the update for the current state.
            $boardMovesData = $this->gameEngine->getBoardMovesData($game);
            $this->gameUpdatePublisher->publishGameUpdate($message->gameUuid, $boardMovesData);

            return;
        }

        $boardMovesData = $this->gameEngine->aiMove($game);
        $this->gameUpdatePublisher->publishGameUpdate($message->gameUuid, $boardMovesData);
    }
}
