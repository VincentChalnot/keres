<?php
declare(strict_types=1);

namespace App\Action;

use App\Engine\BoardTreeManager;
use App\Engine\EngineApi;
use App\Model\BoardMovesData;
use App\Model\MoveData;
use App\Model\OpponentType;
use App\Repository\GameRepository;
use Doctrine\ORM\EntityManagerInterface;
use Symfony\Component\HttpFoundation\JsonResponse;
use Symfony\Component\HttpFoundation\Request;
use Symfony\Component\HttpFoundation\Response;
use Symfony\Component\HttpKernel\Attribute\AsController;
use Symfony\Component\Routing\Attribute\Route;
use Symfony\Component\Uid\Uuid;

#[AsController]
readonly class SubmitMoveAction
{
    public function __construct(
        private GameRepository $gameRepository,
        private BoardTreeManager $boardTreeManager,
        private EntityManagerInterface $entityManager,
        private EngineApi $engineApi,
    ) {
    }

    #[Route(
        path: '/play/{uuid}/move',
        name: 'submit_move',
        methods: ['POST'],
    )]
    public function __(string $uuid, Request $request): Response
    {
        $game = $this->gameRepository->findByUuid(Uuid::fromString($uuid));
        if (!$game) {
            return new JsonResponse(
                ['error' => 'Game not found'],
                Response::HTTP_NOT_FOUND
            );
        }

        if ($game->isGameOver()) {
            return new JsonResponse(
                ['error' => 'Game is already over'],
                Response::HTTP_BAD_REQUEST
            );
        }

        // In AI mode, validate that it's the player's turn
        if (($game->getOpponentType() === OpponentType::AI) && $game->isWhiteTurn() !== $game->isWhite()) {
            // Check if it's the player's turn
            return new JsonResponse(
                ['error' => 'Not your turn'],
                Response::HTTP_BAD_REQUEST
            );
        }

        try {
            $moveData = new MoveData($request->getContent());
        } catch (\Exception $e) {
            return new JsonResponse(
                ['error' => 'Invalid move data: '.$e->getMessage()],
                Response::HTTP_BAD_REQUEST
            );
        }

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

        // If game is in hot seat mode, simply return the mode data
        if ($game->getOpponentType() === OpponentType::HOTSEAT) {
            return $this->getResponse($boardMovesData);
        }

        throw new \RuntimeException('Not implemented');
    }

    private function getResponse(BoardMovesData $boardMovesData): Response
    {
        $boardData = $boardMovesData->boardData;

        return new JsonResponse([
            'success' => true,
            'board' => base64_encode($boardData->data),
            'moves' => base64_encode($boardMovesData->movesData->toBinary()),
            'gameOver' => $boardData->gameOver,
            'whiteWins' => $boardData->whiteWins,
            'draw' => $boardData->draw,
        ]);
    }
}
