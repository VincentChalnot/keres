<?php

declare(strict_types=1);

namespace App\Action;

use App\Form\ContactType;
use Symfony\Component\Form\FormFactoryInterface;
use Symfony\Component\HttpFoundation\RedirectResponse;
use Symfony\Component\HttpFoundation\Request;
use Symfony\Component\HttpKernel\Attribute\AsController;
use Symfony\Component\Mailer\MailerInterface;
use Symfony\Component\Mime\Email;
use Symfony\Component\Routing\Attribute\Route;
use Symfony\Component\Routing\Generator\UrlGeneratorInterface;

#[AsController]
class ContactAction
{
    public function __construct(
        private readonly FormFactoryInterface $formFactory,
        private readonly MailerInterface $mailer,
        private readonly UrlGeneratorInterface $urlGenerator,
    ) {
    }

    #[Route(
        path: '/contact',
        name: 'contact',
        methods: ['GET', 'POST'],
    )]
    public function __invoke(Request $request): array|RedirectResponse
    {
        $form = $this->formFactory->create(ContactType::class);
        $form->handleRequest($request);

        if ($form->isSubmitted() && $form->isValid()) {
            $data = $form->getData();

            $email = (new Email())
                ->from('no-reply@keres.fr')
                ->to('contact@keres.fr')
                ->replyTo($data['email'])
                ->subject('[Keres Contact] '.$data['subject'])
                ->text(\sprintf(
                    "Nom : %s\nE-mail : %s\n\nMessage :\n%s",
                    $data['name'],
                    $data['email'],
                    $data['message'],
                ));

            $this->mailer->send($email);

            return new RedirectResponse(
                $this->urlGenerator->generate('contact', ['sent' => 1])
            );
        }

        return [
            'form' => $form->createView(),
            'sent' => $request->query->getBoolean('sent'),
        ];
    }
}
