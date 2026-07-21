<?php

declare(strict_types=1);

namespace App\Action;

use App\Entity\Feedback;
use App\Entity\User;
use App\Form\FeedbackType;
use Doctrine\ORM\EntityManagerInterface;
use Symfony\Bundle\FrameworkBundle\Controller\AbstractController;
use Symfony\Component\HttpFoundation\RedirectResponse;
use Symfony\Component\HttpFoundation\Request;
use Symfony\Component\HttpKernel\Attribute\AsController;
use Symfony\Component\Routing\Attribute\Route;
use Symfony\Component\Security\Http\Attribute\IsGranted;

#[AsController]
class FeedbackAction extends AbstractController
{
    public function __construct(
        private readonly EntityManagerInterface $entityManager,
    ) {
    }

    #[IsGranted('ROLE_USER')]
    #[Route(
        path: '/feedback',
        name: 'feedback',
        methods: ['GET', 'POST'],
    )]
    public function __invoke(Request $request): RedirectResponse|array
    {
        $form = $this->createForm(FeedbackType::class);
        $form->handleRequest($request);

        if ($form->isSubmitted() && $form->isValid()) {
            $data = $form->getData();

            $user = $this->getUser();

            if (!$user instanceof User) {
                throw $this->createAccessDeniedException('User is required to submit feedback');
            }

            $feedback = new Feedback(
                $data['category'],
                $data['message'],
                $user,
            );

            $this->entityManager->persist($feedback);
            $this->entityManager->flush();

            return $this->redirectToRoute('feedback', ['sent' => 1]);
        }

        return [
            'form' => $form->createView(),
            'sent' => $request->query->getBoolean('sent'),
        ];
    }
}
