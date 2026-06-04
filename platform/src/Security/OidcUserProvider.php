<?php

declare(strict_types=1);

namespace App\Security;

use App\Entity\User;
use App\Entity\UserAuth;
use App\Repository\UserAuthRepository;
use App\Repository\UserRepository;
use Doctrine\ORM\EntityManagerInterface;
use Drenso\OidcBundle\Model\OidcTokens;
use Drenso\OidcBundle\Model\OidcUserData;
use Drenso\OidcBundle\Security\UserProvider\OidcUserProviderInterface;
use Symfony\Component\Security\Core\Exception\UnsupportedUserException;
use Symfony\Component\Security\Core\Exception\UserNotFoundException;
use Symfony\Component\Security\Core\User\UserInterface;

class OidcUserProvider implements OidcUserProviderInterface
{
    private ?string $currentProvider = null;

    public function __construct(
        private readonly UserRepository $userRepository,
        private readonly UserAuthRepository $userAuthRepository,
        private readonly EntityManagerInterface $entityManager,
    ) {
    }

    public function setProvider(string $provider): void
    {
        $this->currentProvider = $provider;
    }

    public function ensureUserExists(string $userIdentifier, OidcUserData $userData, OidcTokens $tokens): void
    {
        $provider = $this->currentProvider ?? 'google';
        $sub = $userData->getSub();
        $email = self::resolveEmail($userData->getEmail(), $provider, $sub, $userIdentifier);
        $displayName = $userData->getFullName() ?: $userData->getDisplayName() ?: null;
        $avatarUrl = $userData->getUserDataString('picture') ?: null;

        $existingAuth = $this->userAuthRepository->findByProviderAndProviderId($provider, $sub);

        if (null !== $existingAuth) {
            $existingAuth->getUser()->setDisplayName($displayName);
            $existingAuth->getUser()->setAvatarUrl($avatarUrl);
        } else {
            $user = $this->userRepository->findByEmail($email);

            if (null === $user) {
                $user = new User($email);
                $user->setDisplayName($displayName);
                $user->setAvatarUrl($avatarUrl);
                $this->entityManager->persist($user);
            } else {
                $user->setDisplayName($displayName);
                $user->setAvatarUrl($avatarUrl);
            }

            $auth = new UserAuth($user, $provider, $sub);
            $this->entityManager->persist($auth);
        }

        $this->entityManager->flush();
    }

    public static function resolveEmail(string $email, string $provider, string $sub, string $fallback = ''): string
    {
        if (empty($email) && 'discord' === $provider) {
            return $sub.'@discord.placeholder';
        }

        if (empty($email)) {
            return $fallback ?: $sub;
        }

        return $email;
    }

    public function loadOidcUser(string $userIdentifier): UserInterface
    {
        return $this->loadUserByIdentifier($userIdentifier);
    }

    public function refreshUser(UserInterface $user): UserInterface
    {
        if (!$user instanceof User) {
            throw new UnsupportedUserException(\sprintf('Instances of "%s" are not supported.', $user::class));
        }

        return $this->loadUserByIdentifier($user->getUserIdentifier());
    }

    public function supportsClass(string $class): bool
    {
        return User::class === $class || is_subclass_of($class, User::class);
    }

    public function loadUserByIdentifier(string $identifier): UserInterface
    {
        $user = $this->userRepository->findByEmail($identifier);

        if (null === $user) {
            $exception = new UserNotFoundException(\sprintf('User "%s" not found.', $identifier));
            $exception->setUserIdentifier($identifier);

            throw $exception;
        }

        return $user;
    }
}
