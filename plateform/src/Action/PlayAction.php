<?php
declare(strict_types=1);

namespace App\Action;

use App\Repository\GameRepository;
use Symfony\Bundle\FrameworkBundle\Controller\AbstractController;
use Symfony\Component\HttpFoundation\Response;
use Symfony\Component\HttpKernel\Attribute\AsController;
use Symfony\Component\Routing\Attribute\Route;
use Symfony\Component\Uid\Uuid;

#[AsController]
class PlayAction extends AbstractController
{
    public function __construct(
        private readonly GameRepository $gameRepository,
    ) {
    }

    #[Route(
        path: '/play/{uuid}',
        name: 'play',
    )]
    public function __(string $uuid): Response
    {
        $game = $this->gameRepository->findByUuid(Uuid::fromString($uuid));
        
        if (!$game) {
            throw $this->createNotFoundException('Game not found');
        }

        // Encode moves to base64
        $movesData = $game->getMovesData();
        $movesBase64 = base64_encode($movesData->toBinary());

        return $this->render('actions/play.html.twig', [
            'game' => $game,
            'moves' => $movesBase64,
        ]);
    }
}
