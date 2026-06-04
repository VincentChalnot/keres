<?php
declare(strict_types=1);

namespace App\Repository;

use App\Entity\UserAuth;
use App\Entity\User;
use Doctrine\Bundle\DoctrineBundle\Repository\ServiceEntityRepository;
use Doctrine\Persistence\ManagerRegistry;
use Symfony\Component\Uid\Uuid;

class UserAuthRepository extends ServiceEntityRepository
{
    public function __construct(ManagerRegistry $registry)
    {
        parent::__construct($registry, UserAuth::class);
    }

    public function findByProviderAndProviderId(string $provider, string $providerId): ?UserAuth
    {
        return $this->createQueryBuilder('ua')
            ->andWhere('ua.provider = :provider')
            ->andWhere('ua.providerId = :providerId')
            ->setParameter('provider', $provider)
            ->setParameter('providerId', $providerId)
            ->getQuery()
            ->getOneOrNullResult();
    }

    public function findByUserAndProvider(Uuid $userId, string $provider): ?UserAuth
    {
        return $this->createQueryBuilder('ua')
            ->andWhere('ua.user = :userId')
            ->andWhere('ua.provider = :provider')
            ->setParameter('userId', $userId)
            ->setParameter('provider', $provider)
            ->getQuery()
            ->getOneOrNullResult();
    }
}