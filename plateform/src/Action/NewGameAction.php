<?php
declare(strict_types=1);

namespace App\Action;

use App\Entity\Game;
use App\Entity\User;
use App\Form\NewGameType;
use App\Repository\GameRepository;
use Doctrine\ORM\EntityManagerInterface;
use Symfony\Bundle\FrameworkBundle\Controller\AbstractController;
use Symfony\Component\DependencyInjection\Attribute\Autowire;
use Symfony\Component\HttpFoundation\RedirectResponse;
use Symfony\Component\HttpFoundation\Request;
use Symfony\Component\HttpKernel\Attribute\AsController;
use Symfony\Component\Routing\Attribute\Route;

#[AsController]
class NewGameAction extends AbstractController
{
    public function __construct(
        private readonly EntityManagerInterface $entityManager,
        private readonly GameRepository $gameRepository,
        #[Autowire(env: 'bool:APP_PUBLIC_MODE')]
        private readonly bool $publicMode = true,
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

            $user = $this->getUser();
            if ($user instanceof User) {
                $game->setOwner($user);
            }

            $this->entityManager->persist($game);
            $this->entityManager->flush();

            return $this->redirectToRoute('play', ['uuid' => $game->getUuid()]);
        }

        if ($this->publicMode) {
            $allGames = [];
        } else {
            $allGames = $this->gameRepository->findAllActive();
        }

        return [
            'form' => $form->createView(),
            'publicMode' => $this->publicMode,
            'inProgressGames' => array_filter($allGames, static fn(Game $g) => !$g->isGameOver()),
            'finishedGames' => array_filter($allGames, static fn(Game $g) => $g->isGameOver()),
        ];
    }
}
