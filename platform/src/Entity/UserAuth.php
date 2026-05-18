<?php

declare(strict_types=1);

namespace App\Entity;

use App\Repository\UserAuthRepository;
use Doctrine\DBAL\Types\Types;
use Doctrine\ORM\Mapping as ORM;
use Symfony\Component\Uid\Uuid;

#[ORM\Entity(repositoryClass: UserAuthRepository::class)]
#[ORM\Table(name: 'user_auth')]
#[ORM\UniqueConstraint(name: 'uniq_user_auth_provider', columns: ['user_id', 'provider'])]
class UserAuth
{
    #[ORM\Id]
    #[ORM\Column(type: 'uuid')]
    private Uuid $id;

    #[ORM\ManyToOne(targetEntity: User::class, inversedBy: 'auths')]
    #[ORM\JoinColumn(name: 'user_id', referencedColumnName: 'id', nullable: false)]
    private User $user;

    #[ORM\Column(type: Types::STRING, length: 32)]
    private string $provider;

    #[ORM\Column(type: Types::STRING, length: 255)]
    private string $providerId;

    #[ORM\Column(type: Types::DATETIME_IMMUTABLE)]
    private \DateTimeImmutable $createdAt;

    public function __construct(User $user, string $provider, string $providerId)
    {
        $this->id = Uuid::v4();
        $this->user = $user;
        $this->provider = $provider;
        $this->providerId = $providerId;
        $this->createdAt = new \DateTimeImmutable();
    }

    public function getId(): Uuid
    {
        return $this->id;
    }

    public function getUser(): User
    {
        return $this->user;
    }

    public function getProvider(): string
    {
        return $this->provider;
    }

    public function getProviderId(): string
    {
        return $this->providerId;
    }

    public function getCreatedAt(): \DateTimeImmutable
    {
        return $this->createdAt;
    }
}
