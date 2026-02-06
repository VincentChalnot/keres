<?php
declare(strict_types=1);

namespace App\Action;

use App\Engine\GameEngine;
use App\Message\ProcessAiMoveMessage;
use App\Model\BoardMovesData;
use App\Model\MoveData;
use App\Model\OpponentType;
use App\Repository\GameRepository;
use Symfony\Component\HttpFoundation\JsonResponse;
use Symfony\Component\HttpFoundation\Request;
use Symfony\Component\HttpFoundation\Response;
use Symfony\Component\HttpKernel\Attribute\AsController;
use Symfony\Component\HttpKernel\EventListener\AbstractSessionListener;
use Symfony\Component\Messenger\MessageBusInterface;
use Symfony\Component\Routing\Attribute\Route;
use Symfony\Component\Uid\Uuid;

#[AsController]
readonly class SubmitMoveAction
{
    public function __construct(
        private GameRepository $gameRepository,
        private MessageBusInterface $messageBus,
        private GameEngine $gameEngine,
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

        $boardMovesData = $this->gameEngine->applyMove($game, $moveData);

        // For hot seat mode, simply return the response synchronously
        if ($game->getOpponentType() === OpponentType::HOTSEAT) {
            return $this->getResponse($boardMovesData);
        }

        // For AI mode, return the response and dispatch async message to process AI move
        if ($game->getOpponentType() === OpponentType::AI) {
            $this->messageBus->dispatch(new ProcessAiMoveMessage($uuid));
        }

        return $this->getResponse($boardMovesData);
    }

    private function getResponse(BoardMovesData $boardMovesData): Response
    {
        $boardData = $boardMovesData->boardData;

        return new JsonResponse(
            [
                'success' => true,
                'board' => base64_encode($boardData->data),
                'moves' => base64_encode($boardMovesData->movesData->toBinary()),
                'gameOver' => $boardData->gameOver,
                'whiteWins' => $boardData->whiteWins,
                'draw' => $boardData->draw,
            ],
            Response::HTTP_OK,
            [
                AbstractSessionListener::NO_AUTO_CACHE_CONTROL_HEADER => true,
            ]
        );
    }
}
