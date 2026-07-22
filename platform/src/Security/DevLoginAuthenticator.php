<?php

declare(strict_types=1);

namespace App\Security;

use App\Entity\User;
use App\Repository\UserRepository;
use Doctrine\ORM\EntityManagerInterface;
use Symfony\Component\HttpFoundation\RedirectResponse;
use Symfony\Component\HttpFoundation\Request;
use Symfony\Component\HttpFoundation\Response;
use Symfony\Component\Routing\RouterInterface;
use Symfony\Component\Security\Core\Authentication\Token\TokenInterface;
use Symfony\Component\Security\Core\Exception\AuthenticationException;
use Symfony\Component\Security\Core\Exception\CustomUserMessageAuthenticationException;
use Symfony\Component\Security\Http\Authenticator\AbstractAuthenticator;
use Symfony\Component\Security\Http\Authenticator\Passport\Badge\UserBadge;
use Symfony\Component\Security\Http\Authenticator\Passport\Passport;
use Symfony\Component\Security\Http\Authenticator\Passport\SelfValidatingPassport;
use Symfony\Component\Security\Http\Util\TargetPathTrait;

/**
 * Dev-only login bypass: authenticate as any user by email, no OIDC round trip.
 *
 * Wired into the `main` firewall in every environment for simplicity, but it
 * is inert outside "dev": `supports()` refuses unless kernel.environment is
 * "dev", and the `/dev/login` route it targets only exists there (see
 * config/routes/dev/dev_login.yaml, loaded exclusively in the dev
 * environment). Both gates must agree for this authenticator to ever run.
 */
class DevLoginAuthenticator extends AbstractAuthenticator
{
    use TargetPathTrait;

    private const string FIREWALL_NAME = 'main';
    private const string ALLOWED_ENVIRONMENT = 'dev';

    public function __construct(
        private readonly UserRepository $userRepository,
        private readonly EntityManagerInterface $entityManager,
        private readonly RouterInterface $router,
        private readonly string $environment,
    ) {
    }

    public function supports(Request $request): ?bool
    {
        return self::ALLOWED_ENVIRONMENT === $this->environment
            && 'dev_login' === $request->attributes->get('_route');
    }

    public function authenticate(Request $request): Passport
    {
        $email = $request->query->get('email');

        if (!\is_string($email) || '' === trim($email)) {
            throw new CustomUserMessageAuthenticationException('Missing "email" query parameter.');
        }

        return new SelfValidatingPassport(new UserBadge($email, function (string $identifier): User {
            $user = $this->userRepository->findByEmail($identifier);

            if (null === $user) {
                $user = new User($identifier);
                $user->setDisplayName($identifier);
                $this->entityManager->persist($user);
                $this->entityManager->flush();
            }

            return $user;
        }));
    }

    public function onAuthenticationSuccess(Request $request, TokenInterface $token, string $firewallName): ?Response
    {
        if ($targetPath = $this->getTargetPath($request->getSession(), self::FIREWALL_NAME)) {
            return new RedirectResponse($targetPath);
        }

        return new RedirectResponse($this->router->generate('new_game'));
    }

    public function onAuthenticationFailure(Request $request, AuthenticationException $exception): ?Response
    {
        return new RedirectResponse($this->router->generate('login'));
    }
}
