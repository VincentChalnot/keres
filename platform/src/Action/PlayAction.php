<?php
declare(strict_types=1);

namespace App\Action;

use App\Repository\GameRepository;
use Symfony\Bundle\FrameworkBundle\Controller\AbstractController;
use Symfony\Component\HttpFoundation\Response;
use Symfony\Component\HttpKernel\Attribute\AsController;
use Symfony\Component\Routing\Attribute\Route;
use Symfony\Component\Uid\Uuid;
use App\Model\OpponentType;
use App\Message\ProcessAiMoveMessage;
use Symfony\Component\Messenger\MessageBusInterface;

#[AsController]
class PlayAction extends AbstractController
{
    public function __construct(
        private readonly GameRepository $gameRepository,
        private readonly MessageBusInterface $messageBus, // Inject message bus
    ) {
    }

    #[Route(
        path: '/play/{uuid}',
        name: 'play',
    )]
    public function __(string $uuid): array
    {
        $game = $this->gameRepository->findByUuid(Uuid::fromString($uuid));
        
        if (!$game) {
            throw $this->createNotFoundException('Game not found');
        }

        // AI auto-move trigger logic
        if (
            $game->getOpponentType() === OpponentType::AI &&
            !$game->isGameOver() &&
            $game->isWhiteTurn() !== $game->isWhite() // It's AI's turn
        ) {
            $this->messageBus->dispatch(
                new ProcessAiMoveMessage(
                    $uuid,
                    $game->getGameMoves()->count(),
                )
            );
        }

        // Encode moves to base64
        $movesData = $game->getMovesData();
        $movesBase64 = base64_encode($movesData->toBinary());

        return [
            'game' => $game,
            'moves' => $movesBase64,
        ];
    }
}
