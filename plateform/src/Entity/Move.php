<?php
declare(strict_types=1);

namespace App\Entity;

use Doctrine\DBAL\Types\Types;
use Doctrine\ORM\Mapping as ORM;

#[ORM\Entity]
#[ORM\Table(name: 'move')]
class Move
{
    #[ORM\Id]
    #[ORM\GeneratedValue]
    #[ORM\Column(type: Types::BIGINT)]
    private ?string $id = null;

    #[ORM\Column(type: Types::DATETIME_IMMUTABLE)]
    private ?\DateTimeImmutable $createdAt = null;

    #[ORM\Column(type: Types::BINARY, length: 2)]
    private ?string $move = null; // u16 encoded as binary

    #[ORM\ManyToOne(targetEntity: Game::class, inversedBy: 'moves')]
    #[ORM\JoinColumn(nullable: false)]
    private ?Game $game = null;

    public function __construct()
    {
        $this->createdAt = new \DateTimeImmutable();
    }

    public function getId(): ?string
    {
        return $this->id;
    }

    public function getCreatedAt(): ?\DateTimeImmutable
    {
        return $this->createdAt;
    }

    public function setCreatedAt(\DateTimeImmutable $createdAt): self
    {
        $this->createdAt = $createdAt;
        return $this;
    }

    public function getMove(): ?string
    {
        return $this->move;
    }

    public function setMove(string $move): self
    {
        $this->move = $move;
        return $this;
    }

    /**
     * Set move from u16 integer
     */
    public function setMoveFromU16(int $moveU16): self
    {
        // Pack the u16 into 2 bytes (little endian)
        $this->move = pack('v', $moveU16);
        return $this;
    }

    /**
     * Get move as u16 integer
     * @throws \RuntimeException if move data is not set
     */
    public function getMoveAsU16(): int
    {
        if ($this->move === null) {
            throw new \RuntimeException('Move data is not set');
        }
        // Unpack 2 bytes as little endian u16
        $unpacked = unpack('v', $this->move);
        if ($unpacked === false) {
            throw new \RuntimeException('Failed to unpack move data');
        }
        return $unpacked[1] ?? 0;
    }

    public function getGame(): ?Game
    {
        return $this->game;
    }

    public function setGame(?Game $game): self
    {
        $this->game = $game;
        return $this;
    }
}
