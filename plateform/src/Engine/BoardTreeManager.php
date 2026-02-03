<?php
declare(strict_types=1);

namespace App\Engine;

use App\Entity\BoardPosition;
use App\Entity\Game;
use App\Entity\GameMove;
use App\Entity\Move;
use App\Model\BoardData;
use App\Model\BoardMovesData;
use App\Model\MoveData;
use App\Model\MovesData;
use Doctrine\ORM\EntityManagerInterface;
use Doctrine\ORM\EntityRepository;
use Doctrine\Persistence\ManagerRegistry;

readonly class BoardTreeManager
{
    public function __construct(
        private EngineApi $engineApi,
        private ManagerRegistry $managerRegistry,
    ) {
    }

    public function getGameMove(Game $game, BoardMovesData $boardMovesData): GameMove
    {
        $toBoardPosition = $this->getBoardPosition($boardMovesData->boardData);

        $lastMove = $game->getGameMoves()->last();
        if (!$lastMove) {
            $fromBoardPosition = $this->getRootBoardPosition();
        } else {
            $fromBoardPosition = $lastMove->getMove()->getToBoardPosition();
        }
        $lastMoveData = $boardMovesData->movesData->getMoves()->last();
        if (!$lastMoveData instanceof MoveData) {
            throw new \RuntimeException('No moves found in BoardMovesData');
        }
        $move = $this->getMove($lastMoveData, $fromBoardPosition, $toBoardPosition);

        return $game->addMove($move);
    }

    public function getMove(MoveData $moveData, BoardPosition $fromBoardPosition, BoardPosition $toBoardPosition): Move
    {
        $repo = $this->getRepository(Move::class);
        $move = $repo->findOneBy([
            'moveData' => $moveData->data,
            'fromBoardPosition' => $fromBoardPosition,
        ]);
        if ($move) {
            if ($move->getToBoardPosition() !== $toBoardPosition) {
                throw new \RuntimeException('Inconsistent toBoardPosition for existing Move');
            }
        } else {
            $move = new Move($moveData, $fromBoardPosition, $toBoardPosition);

            $em = $this->getEntityManager();
            $em->wrapInTransaction(function (EntityManagerInterface $em) use ($toBoardPosition, $move) {
                $em->persist($toBoardPosition);
                $em->persist($move);
            });
        }

        return $move;

    }

    private function getEntityManager(): EntityManagerInterface
    {
        $em = $this->managerRegistry->getManagerForClass(Move::class);
        if (!$em) {
            throw new \RuntimeException('No entity manager found for Move class');
        }

        return $em;
    }

    private function getRepository(string $entityClass): EntityRepository
    {
        $em = $this->getEntityManager();
        $repository = $em->getRepository($entityClass);
        if (!$repository instanceof EntityRepository) {
            throw new \RuntimeException("Repository for {$entityClass} is not an EntityRepository");
        }

        return $repository;
    }

    private function getRootBoardPosition(): BoardPosition
    {
        $rootBoardData = $this->engineApi->replayMoves(new MovesData());

        return $this->getBoardPosition($rootBoardData);
    }

    private function getBoardPosition(BoardData $boardData): BoardPosition
    {
        $repo = $this->getRepository(BoardPosition::class);
        $boardPosition = $repo->findOneBy(['boardPositionData' => $boardData->getPositionData()]);
        if (!$boardPosition) {
            $boardPosition = new BoardPosition($boardData);
            $em = $this->managerRegistry->getManager();
            $em->wrapInTransaction(function (EntityManagerInterface $em) use ($boardPosition) {
                $em->persist($boardPosition);
            });
        }

        return $boardPosition;
    }
}
