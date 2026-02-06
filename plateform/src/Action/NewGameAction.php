<?php
declare(strict_types=1);

namespace App\Action;

use App\Entity\Game;
use App\Form\NewGameType;
use App\Repository\GameRepository;
use Doctrine\ORM\EntityManagerInterface;
use Symfony\Bundle\FrameworkBundle\Controller\AbstractController;
use Symfony\Component\HttpFoundation\RedirectResponse;
use Symfony\Component\HttpFoundation\Request;
use Symfony\Component\HttpFoundation\Response;
use Symfony\Component\HttpKernel\Attribute\AsController;
use Symfony\Component\Routing\Attribute\Route;

#[AsController]
class NewGameAction extends AbstractController
{
    public function __construct(
        private readonly EntityManagerInterface $entityManager,
        private readonly GameRepository $gameRepository,
    ) {
    }

    #[Route(
        path: '/play',
        name: 'new_game',
        methods: ['GET', 'POST'],
    )]
    public function __(Request $request): RedirectResponse|array
    {
        $form = $this->createForm(NewGameType::class);
        $form->handleRequest($request);

        if ($form->isSubmitted() && $form->isValid()) {
            $data = $form->getData();

            $game = new Game();
            $game->setIsWhite(
                match ($data['playerSide']) {
                    'white' => true,
                    'black' => false,
                    'random' => (bool) random_int(0, 1),
                }
            );
            $game->setOpponentType($data['opponentType']);

            $this->entityManager->persist($game);
            $this->entityManager->flush();

            return $this->redirectToRoute('play', ['uuid' => $game->getUuid()]);
        }

        return [
            'form' => $form->createView(),
            'games' => $this->gameRepository->findAll(),
        ];
    }
}
