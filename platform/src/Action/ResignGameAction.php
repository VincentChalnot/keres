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
class ResignGameAction extends AbstractController
{
    public function __construct(
        private readonly GameRepository $gameRepository,
        private readonly EntityManagerInterface $entityManager,
    ) {
    }

    #[Route(
        path: '/play/{uuid}/resign',
        name: 'resign_game',
        methods: ['POST'],
    )]
    public function __(string $uuid): Response
    {
        $game = $this->gameRepository->findByUuid(Uuid::fromString($uuid));

        if (!$game) {
            throw $this->createNotFoundException('Game not found');
        }

        if ($game->isGameOver()) {
            return $this->redirectToRoute('new_game');
        }

        $game->setGameOverAt(new \DateTimeImmutable());
        $game->setWhiteWins(!$game->isWhite());
        $game->setDraw(false);

        $this->entityManager->persist($game);
        $this->entityManager->flush();

        return $this->redirectToRoute('new_game');
    }
}
