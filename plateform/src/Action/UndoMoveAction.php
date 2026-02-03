<?php
declare(strict_types=1);

namespace App\Action;

use App\Repository\GameRepository;
use Doctrine\ORM\EntityManagerInterface;
use Symfony\Bundle\FrameworkBundle\Controller\AbstractController;
use Symfony\Component\HttpFoundation\Response;
use Symfony\Component\HttpKernel\Attribute\AsController;
use Symfony\Component\Routing\Attribute\Route;
use Symfony\Component\Uid\Uuid;

#[AsController]
class UndoMoveAction extends AbstractController
{
    public function __construct(
        private readonly GameRepository $gameRepository,
        private readonly EntityManagerInterface $entityManager,
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
            return $this->json(['error' => 'Game not found'], Response::HTTP_NOT_FOUND);
        }

        if ($game->isGameOver()) {
            return $this->json(['error' => 'Game is already over'], Response::HTTP_BAD_REQUEST);
        }

        $moves = $game->getGameMoves();
        $moveCount = count($moves);
        
        if ($moveCount === 0) {
            return $this->json(['error' => 'No moves to undo'], Response::HTTP_BAD_REQUEST);
        }

        // Determine how many moves to remove based on game type
        $movesToRemove = 1;
        if ($game->getOpponentType() === 'ai' && $moveCount >= 2) {
            // In AI mode, undo both player move and AI response
            $movesToRemove = 2;
        }

        // Remove the moves
        for ($i = 0; $i < $movesToRemove && count($moves) > 0; $i++) {
            $lastMove = $moves->last();
            if ($lastMove) {
                $game->removeMove($lastMove);
                $this->entityManager->remove($lastMove);
            }
        }

        // Clear game over state if it was set
        if ($game->isGameOver()) {
            $game->setGameOverAt(null);
            $game->setWhiteWins(false);
            $game->setDraw(false);
        }

        $this->entityManager->flush();

        // Return the updated moves list
        $movesData = [];
        foreach ($game->getGameMoves() as $moveEntity) {
            $movesData[] = $moveEntity->getMoveAsU16();
        }

        return $this->json([
            'success' => true,
            'moves' => $movesData,
        ]);
    }
}
