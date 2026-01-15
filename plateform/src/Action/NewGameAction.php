<?php
declare(strict_types=1);

namespace App\Action;

use App\Entity\Game;
use App\Form\NewGameType;
use Doctrine\ORM\EntityManagerInterface;
use Symfony\Bundle\FrameworkBundle\Controller\AbstractController;
use Symfony\Component\HttpFoundation\Request;
use Symfony\Component\HttpFoundation\Response;
use Symfony\Component\HttpKernel\Attribute\AsController;
use Symfony\Component\Routing\Attribute\Route;

#[AsController]
class NewGameAction extends AbstractController
{
    public function __construct(
        private readonly EntityManagerInterface $entityManager,
    ) {
    }

    #[Route(
        path: '/play',
        name: 'new_game',
        methods: ['GET', 'POST'],
    )]
    public function __(Request $request): Response
    {
        $form = $this->createForm(NewGameType::class);
        $form->handleRequest($request);

        if ($form->isSubmitted() && $form->isValid()) {
            $data = $form->getData();
            
            // Resolve random side
            $playerSide = $data['playerSide'];
            if ($playerSide === 'random') {
                $playerSide = random_int(0, 1) === 0 ? 'white' : 'black';
            }

            $game = new Game();
            $game->setPlayerSide($playerSide);
            $game->setOpponentType($data['opponentType']);

            $this->entityManager->persist($game);
            $this->entityManager->flush();

            return $this->redirectToRoute('play', ['uuid' => $game->getUuid()]);
        }

        return $this->render('actions/new_game.html.twig', [
            'form' => $form->createView(),
        ]);
    }
}
