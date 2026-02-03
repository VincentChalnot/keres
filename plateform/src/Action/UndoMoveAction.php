<?php
declare(strict_types=1);

namespace App\Action;

use App\Entity\GameMove;
use App\Model\OpponentType;
use App\Repository\GameRepository;
use Doctrine\ORM\EntityManagerInterface;
use Symfony\Component\HttpFoundation\JsonResponse;
use Symfony\Component\HttpFoundation\Response;
use Symfony\Component\HttpKernel\Attribute\AsController;
use Symfony\Component\Routing\Attribute\Route;
use Symfony\Component\Uid\Uuid;

#[AsController]
readonly class UndoMoveAction
{
    public function __construct(
        private GameRepository $gameRepository,
        private EntityManagerInterface $entityManager,
    ) {
    }

    #[Route(
        path: '/play/{uuid}/undo',
        name: 'undo_move',
        methods: ['POST'],
    )]
    public function __(string $uuid): Response
    {
        $game = $this->gameRepository->findByUuid(Uuid::fromString($uuid));

        if (!$game) {
            return new JsonResponse([
                'success' => false,
                'error' => 'Game not found',
            ], Response::HTTP_NOT_FOUND);
        }

        if ($game->getGameMoves()->isEmpty()) {
            return new JsonResponse([
                'success' => false,
                'error' => 'No moves to undo',
            ], Response::HTTP_BAD_REQUEST);
        }

        $this->entityManager->wrapInTransaction(function (EntityManagerInterface $em) use ($game) {
            $lastMove = $this->removeLastMoveFromGame($game);
            $em->remove($lastMove);
            if ($game->getOpponentType() === OpponentType::AI) {
                // In AI mode, undo both player move and AI response
                $aiLastMove = $this->removeLastMoveFromGame($game);
                $em->remove($aiLastMove);
            }
            $game->setGameOverAt(null);
            $game->setWhiteWins(false);
            $game->setDraw(false);
            $em->persist($game);
        });

        return new Response(base64_encode($game->getMovesData()->toBinary()));
    }

    private function removeLastMoveFromGame($game): GameMove
    {
        $lastElement = $game->getGameMoves()->last();
        if ($lastElement === false) {
            throw new \LogicException('Unexpected error retrieving last move.');
        }
        $game->getGameMoves()->removeElement($lastElement);

        return $lastElement;
    }
}
