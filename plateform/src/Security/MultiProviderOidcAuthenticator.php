<?php
declare(strict_types=1);

namespace App\Security;

use Drenso\OidcBundle\OidcClientLocator;
use Drenso\OidcBundle\Security\Exception\OidcAuthenticationException;
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

class MultiProviderOidcAuthenticator extends AbstractAuthenticator implements AuthenticationEntryPointInterface
{
    public const SESSION_PROVIDER_KEY = '_oidc_provider';

    public function __construct(
        private readonly OidcClientLocator $oidcClientLocator,
        private readonly OidcUserProvider $oidcUserProvider,
        private readonly RouterInterface $router,
    ) {
    }

    public function supports(Request $request): ?bool
    {
        return $request->attributes->get('_route') === 'oidc_login_check'
            && $request->query->has('code')
            && $request->query->has('state');
    }

    public function authenticate(Request $request): Passport
    {
        $provider = $request->getSession()->get(self::SESSION_PROVIDER_KEY, 'google');
        $this->oidcUserProvider->setProvider($provider);

        try {
            $client = $this->oidcClientLocator->getClient($provider);
            $tokens = $client->authenticate($request);
            $userData = $client->retrieveUserInfo($tokens);

            $userIdentifier = $userData->getUserDataString('sub');
            if (empty($userIdentifier)) {
                throw new AuthenticationException('No "sub" claim found in OIDC user data.');
            }

            $this->oidcUserProvider->ensureUserExists($userIdentifier, $userData, $tokens);

            $email = $userData->getEmail();
            if (empty($email) && $provider === 'discord') {
                $email = $userIdentifier . '@discord.placeholder';
            }
            if (empty($email)) {
                $email = $userIdentifier;
            }

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

        return new RedirectResponse($this->router->generate('index'));
    }

    public function onAuthenticationFailure(Request $request, AuthenticationException $exception): ?Response
    {
        $request->getSession()->remove(self::SESSION_PROVIDER_KEY);

        return new RedirectResponse($this->router->generate('login'));
    }

    public function start(Request $request, ?AuthenticationException $authException = null): Response
    {
        return new RedirectResponse($this->router->generate('login'));
    }
}
