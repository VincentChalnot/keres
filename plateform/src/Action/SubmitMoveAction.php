<?php
declare(strict_types=1);

namespace App\Action;

use App\Engine\BoardTreeManager;
use App\Engine\EngineApi;
use App\Message\ProcessAiMoveMessage;
use App\Model\BoardMovesData;
use App\Model\MoveData;
use App\Model\OpponentType;
use App\Repository\GameRepository;
use App\Service\GameUpdatePublisher;
use Doctrine\ORM\EntityManagerInterface;
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
        private BoardTreeManager $boardTreeManager,
        private EntityManagerInterface $entityManager,
        private EngineApi $engineApi,
        private MessageBusInterface $messageBus,
        private GameUpdatePublisher $gameUpdatePublisher,
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

        // For hot seat mode, simply return the response synchronously
        if ($game->getOpponentType() === OpponentType::HOTSEAT) {
            return $this->getResponse($boardMovesData);
        }

        // For AI mode, return the response and dispatch async message to process AI move
        if ($game->getOpponentType() === OpponentType::AI) {
            $response = $this->getResponse($boardMovesData);
            
            // Dispatch message after response is sent
            // We need to ensure the message is dispatched after the response
            $response->headers->set(AbstractSessionListener::NO_AUTO_CACHE_CONTROL_HEADER, '1');
            $this->messageBus->dispatch(new ProcessAiMoveMessage($uuid));
            
            return $response;
        }

        // For future multiplayer mode, publish to Mercure and return response
        // This will be implemented when multiplayer is added
        throw new \RuntimeException('Multiplayer mode not yet implemented');
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
