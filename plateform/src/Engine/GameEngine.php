<?php

namespace App\Engine;

use App\Entity\Game;
use App\Model\BoardMovesData;
use App\Model\MoveData;
use Doctrine\DBAL\TransactionIsolationLevel;
use Doctrine\ORM\EntityManagerInterface;
use Doctrine\ORM\OptimisticLockException;

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

        $expectedVersion = $game->getVersion();

        $connection = $this->entityManager->getConnection();
        $previousIsolation = $connection->getTransactionIsolation();
        $connection->setTransactionIsolation(TransactionIsolationLevel::SERIALIZABLE);

        try {
            $connection->beginTransaction();
            try {
                $newMove = $this->boardTreeManager->getGameMove($game, $boardMovesData);
                $this->entityManager->persist($newMove);

                // Update game state if game is over
                if ($boardData->gameOver) {
                    $game->setGameOverAt(new \DateTimeImmutable());
                    $game->setWhiteWins($boardData->whiteWins);
                    $game->setDraw($boardData->draw);
                }

                $this->entityManager->flush();

                // For non-game-over moves, the Game entity is not dirty so Doctrine
                // does not UPDATE it (and thus does not check/bump the version).
                // Perform an atomic version check + bump via native SQL.
                if (!$boardData->gameOver) {
                    $tableName = $this->entityManager->getClassMetadata(Game::class)->getTableName();
                    $rowsAffected = $connection->executeStatement(
                        'UPDATE '.$tableName.' SET version = version + 1 WHERE id = :id AND version = :version',
                        ['id' => $game->getId(), 'version' => $expectedVersion]
                    );
                    if ($rowsAffected === 0) {
                        throw OptimisticLockException::lockFailed($game);
                    }
                }

                $connection->commit();
            } catch (\Throwable $e) {
                $connection->rollBack();
                throw $e;
            }
        } finally {
            $connection->setTransactionIsolation($previousIsolation);
        }

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

        // Get AI move
        $aiMoveData = $this->engineApi->aiMove($movesData);

        // Apply AI move
        return $this->applyMove($game, $aiMoveData);
    }
}
