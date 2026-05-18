<?php

declare(strict_types=1);

namespace App\Security;

use Drenso\OidcBundle\OidcClientLocator;
use Drenso\OidcBundle\Security\Exception\OidcAuthenticationException;
use Psr\Log\LoggerInterface;
use Symfony\Component\HttpFoundation\RedirectResponse;
use Symfony\Component\HttpFoundation\Request;
use Symfony\Component\HttpFoundation\Response;
use Symfony\Component\Routing\RouterInterface;
use Symfony\Component\Security\Core\Authentication\Token\TokenInterface;
use Symfony\Component\Security\Core\Exception\AuthenticationException;
use Symfony\Component\Security\Http\Authenticator\AbstractAuthenticator;
use Symfony\Component\Security\Http\Authenticator\Passport\Badge\UserBadge;
use Symfony\Component\Security\Http\Authenticator\Passport\Passport;
use Symfony\Component\Security\Http\Authenticator\Passport\SelfValidatingPassport;
use Symfony\Component\Security\Http\EntryPoint\AuthenticationEntryPointInterface;
use Symfony\Component\Security\Http\Util\TargetPathTrait;

class MultiProviderOidcAuthenticator extends AbstractAuthenticator implements AuthenticationEntryPointInterface
{
    use TargetPathTrait;

    public const string SESSION_PROVIDER_KEY = '_oidc_provider';
    private const string FIREWALL_NAME = 'main';

    public function __construct(
        private readonly OidcClientLocator $oidcClientLocator,
        private readonly OidcUserProvider $oidcUserProvider,
        private readonly RouterInterface $router,
        private readonly LoggerInterface $logger,
    ) {
    }

    public function supports(Request $request): ?bool
    {
        return 'oidc_login_check' === $request->attributes->get('_route')
            && $request->query->has('code')
            && $request->query->has('state');
    }

    public function authenticate(Request $request): Passport
    {
        $provider = $request->getSession()->get(self::SESSION_PROVIDER_KEY, 'google');
        $this->oidcUserProvider->setProvider($provider);
        $this->logger->info('OIDC Authentication started', ['provider' => $provider]);

        try {
            $this->logger->info('Getting OIDC client for provider: ' . $provider);
            $client = $this->oidcClientLocator->getClient($provider);

            $this->logger->info('Authenticating with OIDC client');
            $tokens = $client->authenticate($request);
            $userData = $client->retrieveUserInfo($tokens);

            $userIdentifier = $userData->getUserDataString('sub');
            $this->logger->info('User identifier from sub claim', ['sub' => $userIdentifier]);

            if (empty($userIdentifier)) {
                throw new AuthenticationException('No "sub" claim found in OIDC user data.');
            }

            $this->oidcUserProvider->ensureUserExists($userIdentifier, $userData, $tokens);

            $email = OidcUserProvider::resolveEmail($userData->getEmail(), $provider, $userIdentifier, $userIdentifier);
            $this->logger->info('Email resolved', ['email' => $email]);

            return new SelfValidatingPassport(new UserBadge(
                $email,
                $this->oidcUserProvider->loadOidcUser(...),
            ));
        } catch (\Throwable $e) {
            throw new OidcAuthenticationException('OIDC authentication failed', $e);
        }
    }

    public function onAuthenticationSuccess(Request $request, TokenInterface $token, string $firewallName): ?Response
    {
        $request->getSession()->remove(self::SESSION_PROVIDER_KEY);

        $targetPath = $this->getTargetPath($request->getSession(), self::FIREWALL_NAME);

        if ($targetPath) {
            return new RedirectResponse($targetPath);
        }

        return new RedirectResponse($this->router->generate('index'));
    }

    public function onAuthenticationFailure(Request $request, AuthenticationException $exception): ?Response
    {
        $request->getSession()->remove(self::SESSION_PROVIDER_KEY);

        return new RedirectResponse($this->router->generate('login'));
    }

    public function start(Request $request, ?AuthenticationException $authException = null): Response
    {
        $this->saveTargetPath($request->getSession(), self::FIREWALL_NAME, $request->getUri());

        return new RedirectResponse($this->router->generate('login'));
    }
}
