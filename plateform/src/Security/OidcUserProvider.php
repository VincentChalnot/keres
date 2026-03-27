<?php
declare(strict_types=1);

namespace App\Security;

use App\Entity\User;
use App\Repository\UserRepository;
use Drenso\OidcBundle\Model\OidcTokens;
use Drenso\OidcBundle\Model\OidcUserData;
use Drenso\OidcBundle\Security\UserProvider\OidcUserProviderInterface;
use Doctrine\ORM\EntityManagerInterface;
use Symfony\Component\Security\Core\Exception\UnsupportedUserException;
use Symfony\Component\Security\Core\Exception\UserNotFoundException;
use Symfony\Component\Security\Core\User\UserInterface;

/**
 * @implements OidcUserProviderInterface<User>
 */
class OidcUserProvider implements OidcUserProviderInterface
{
    private ?string $currentProvider = null;

    public function __construct(
        private readonly UserRepository $userRepository,
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

        $user = $this->userRepository->findByProviderAndProviderId($provider, $sub);

        if ($user === null) {
            // First login: create a new user
            $user = new User($provider, $sub, $email);
            $user->setDisplayName($displayName);
            $user->setAvatarUrl($avatarUrl);
            $this->entityManager->persist($user);
        } else {
            // Subsequent logins: update displayName and avatarUrl if changed, do NOT change email
            $user->setDisplayName($displayName);
            $user->setAvatarUrl($avatarUrl);
        }

        $this->entityManager->flush();
    }

    /**
     * Resolve the user email from OIDC data, handling the case where Discord
     * may not provide an email address.
     *
     * TODO: handle missing Discord email gracefully in the UI
     */
    public static function resolveEmail(string $email, string $provider, string $sub, string $fallback = ''): string
    {
        if (empty($email) && $provider === 'discord') {
            return $sub . '@discord.placeholder';
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
            throw new UnsupportedUserException(sprintf('Instances of "%s" are not supported.', $user::class));
        }

        return $this->loadUserByIdentifier($user->getUserIdentifier());
    }

    public function supportsClass(string $class): bool
    {
        return $class === User::class || is_subclass_of($class, User::class);
    }

    public function loadUserByIdentifier(string $identifier): UserInterface
    {
        $user = $this->userRepository->findByEmail($identifier);

        if ($user === null) {
            $exception = new UserNotFoundException(sprintf('User "%s" not found.', $identifier));
            $exception->setUserIdentifier($identifier);

            throw $exception;
        }

        return $user;
    }
}
