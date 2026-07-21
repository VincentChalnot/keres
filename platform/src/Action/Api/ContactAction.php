<?php

declare(strict_types=1);

namespace App\Action\Api;

use Symfony\Component\HttpFoundation\JsonResponse;
use Symfony\Component\HttpFoundation\Request;
use Symfony\Component\HttpFoundation\Response;
use Symfony\Component\HttpKernel\Attribute\AsController;
use Symfony\Component\Mailer\MailerInterface;
use Symfony\Component\Mime\Email;
use Symfony\Component\RateLimiter\RateLimiterFactory;
use Symfony\Component\Routing\Attribute\Route;

#[AsController]
readonly class ContactAction
{
    public function __construct(
        private MailerInterface $mailer,
        private RateLimiterFactory $contactLimiterFactory,
    ) {}

    #[Route(
        path: '/api/contact',
        name: 'api_contact',
        methods: ['POST', 'OPTIONS'],
    )]
    public function __invoke(Request $request): Response
    {
        if ('OPTIONS' === $request->getMethod()) {
            return new JsonResponse(null, Response::HTTP_NO_CONTENT);
        }

        // Honeypot: hidden "website" field must be empty
        $payload = json_decode($request->getContent(), true) ?? [];

        if (!empty($payload['website'] ?? '')) {
            // Pretend success — do not leak to bots that we detected them
            return new JsonResponse(['success' => true], Response::HTTP_OK);
        }

        foreach (['name', 'email', 'subject', 'message'] as $required) {
            if (empty($payload[$required])) {
                return new JsonResponse(['error' => "missing field: $required"], Response::HTTP_UNPROCESSABLE_ENTITY);
            }
        }

        $limiter = $this->contactLimiterFactory->create($request->getClientIp());
        if (false === $limiter->consume(1)->isAccepted()) {
            return new JsonResponse(['error' => 'rate limit exceeded'], Response::HTTP_TOO_MANY_REQUESTS);
        }

        $email = (new Email())
            ->from('no-reply@keres.fr')
            ->to('contact@keres.fr')
            ->replyTo($payload['email'])
            ->subject('[Keres Contact] '.$payload['subject'])
            ->text(\sprintf(
                "Nom : %s\nE-mail : %s\n\nMessage :\n%s",
                $payload['name'],
                $payload['email'],
                $payload['message'],
            ));

        $this->mailer->send($email);

        return new JsonResponse(['success' => true], Response::HTTP_OK);
    }
}
