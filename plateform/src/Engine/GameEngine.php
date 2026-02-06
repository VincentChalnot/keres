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
}
