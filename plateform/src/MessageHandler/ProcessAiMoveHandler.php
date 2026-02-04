<?php
declare(strict_types=1);

namespace App\MessageHandler;

use App\Engine\BoardTreeManager;
use App\Engine\EngineApi;
use App\Message\ProcessAiMoveMessage;
use App\Model\BoardMovesData;
use App\Repository\GameRepository;
use App\Service\GameUpdatePublisher;
use Doctrine\ORM\EntityManagerInterface;
use Symfony\Component\Messenger\Attribute\AsMessageHandler;
use Symfony\Component\Uid\Uuid;

#[AsMessageHandler]
readonly class ProcessAiMoveHandler
{
    public function __construct(
        private GameRepository $gameRepository,
        private EngineApi $engineApi,
        private BoardTreeManager $boardTreeManager,
        private EntityManagerInterface $entityManager,
        private GameUpdatePublisher $gameUpdatePublisher,
    ) {
    }

    public function __invoke(ProcessAiMoveMessage $message): void
    {
        $game = $this->gameRepository->findByUuid(Uuid::fromString($message->gameUuid));
        if (!$game) {
            throw new \RuntimeException('Game not found: '.$message->gameUuid);
        }

        if ($game->isGameOver()) {
            // Game is already over, nothing to do
            return;
        }

        // Get current board state
        $movesData = $game->getMovesData();
        $boardData = $this->engineApi->replayMoves($movesData);

        // Get AI move
        $aiMoveData = $this->engineApi->aiMove($boardData);

        // Apply AI move
        $movesData->addMove($aiMoveData);
        $newBoardData = $this->engineApi->replayMoves($movesData);
        $boardMovesData = new BoardMovesData($newBoardData, $movesData);

        // Save the move to database
        $this->entityManager->wrapInTransaction(
            function (EntityManagerInterface $em) use ($game, $boardMovesData, $newBoardData) {
                $newMove = $this->boardTreeManager->getGameMove($game, $boardMovesData);
                $em->persist($newMove);

                // Update game state if game is over
                if ($newBoardData->gameOver) {
                    $game->setGameOverAt(new \DateTimeImmutable());
                    $game->setWhiteWins($newBoardData->whiteWins);
                    $game->setDraw($newBoardData->draw);
                    $em->persist($game);
                }
            }
        );

        // Publish update to Mercure
        $this->gameUpdatePublisher->publishGameUpdate($message->gameUuid, $boardMovesData);
    }
}
