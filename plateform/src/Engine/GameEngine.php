<?php

namespace App\Engine;

use App\Entity\Game;
use App\Model\BoardMovesData;
use App\Model\MoveData;
use Doctrine\ORM\EntityManagerInterface;

readonly class GameEngine
{
    public function __construct(
        private BoardTreeManager $boardTreeManager,
        private EntityManagerInterface $entityManager,
        private EngineApi $engineApi,
    ) {
    }

    /**
     * Returns a BoardMovesData for the current game state (board after all moves played).
     */
    public function getBoardMovesData(Game $game): BoardMovesData
    {
        $movesData = $game->getMovesData();
        $boardData = $this->engineApi->replayMoves($movesData);
        return new BoardMovesData($boardData, $movesData);
    }

    public function applyMove(Game $game, MoveData $moveData): BoardMovesData
    {
        $movesData = $game->getMovesData();
        $movesData->addMove($moveData);
        $boardData = $this->engineApi->replayMoves($movesData);
        $boardMovesData = new BoardMovesData($boardData, $movesData);

        $this->entityManager->wrapInTransaction(
            function (EntityManagerInterface $em) use ($game, $boardMovesData, $boardData) {
                $newMove = $this->boardTreeManager->getGameMove($game, $boardMovesData);
                $em->persist($newMove);

                // Update game state if game is over
                if ($boardData->gameOver) {
                    $game->setGameOverAt(new \DateTimeImmutable());
                    $game->setWhiteWins($boardData->whiteWins);
                    $game->setDraw($boardData->draw);
                    $em->persist($game);
                }
            }
        );

        return $boardMovesData;
    }

    public function aiMove(Game $game): BoardMovesData
    {
        if ($game->isGameOver()) {
            // Game is already over, nothing to do
            throw new \RuntimeException('Game is over.');
        }

        // Get current board state
        $movesData = $game->getMovesData();
        $boardData = $this->engineApi->replayMoves($movesData);

        // Get AI move
        $aiMoveData = $this->engineApi->aiMove($boardData);

        // Apply AI move
        return $this->applyMove($game, $aiMoveData);
    }
}
