<?php

declare(strict_types=1);

namespace App\Action;

use App\Security\MultiProviderOidcAuthenticator;
use App\Security\OidcUserProvider;
use Drenso\OidcBundle\OidcClientLocator;
use Symfony\Bundle\FrameworkBundle\Controller\AbstractController;
use Symfony\Component\HttpFoundation\RedirectResponse;
use Symfony\Component\HttpFoundation\Request;
use Symfony\Component\HttpFoundation\Response;
use Symfony\Component\HttpKernel\Attribute\AsController;
use Symfony\Component\Routing\Attribute\Route;

#[AsController]
class LoginAction extends AbstractController
{
    public function __construct(
        private readonly OidcClientLocator $oidcClientLocator,
        private readonly OidcUserProvider $oidcUserProvider,
    ) {
    }

    #[Route(path: '/login', name: 'login', methods: ['GET'])]
    public function login(): Response
    {
        if ($this->getUser()) {
            return $this->redirectToRoute('new_game');
        }

        return $this->render('security/login.html.twig');
    }

    #[Route(path: '/login/{provider}', name: 'oidc_login', methods: ['GET'])]
    public function oidcLogin(string $provider, Request $request): RedirectResponse
    {
        $validProviders = ['google', 'discord', 'facebook'];

        if (!\in_array($provider, $validProviders, true)) {
            throw $this->createNotFoundException('Unknown provider.');
        }

        $request->getSession()->set(MultiProviderOidcAuthenticator::SESSION_PROVIDER_KEY, $provider);
        $this->oidcUserProvider->setProvider($provider);

        $client = $this->oidcClientLocator->getClient($provider);

        $scopes = match ($provider) {
            'google' => ['openid', 'email', 'profile'],
            'facebook' => ['openid', 'email', 'public_profile'],
            'discord' => ['openid', 'identify', 'email'],
        };

        return $client->generateAuthorizationRedirect(scopes: $scopes);
    }

    #[Route(path: '/auth/callback', name: 'oidc_login_check', methods: ['GET'])]
    public function check(): never
    {
        // This route is handled by the Drenso OIDC authenticator
        throw new \LogicException('This should be handled by the OIDC authenticator.');
    }

    #[Route(path: '/logout', name: 'logout', methods: ['GET'])]
    public function logout(): never
    {
        // This route is handled by the Symfony security component
        throw new \LogicException('This should be handled by Symfony\'s logout handler.');
    }
}
