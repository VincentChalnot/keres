<?php
declare(strict_types=1);

namespace App\Entity;

use Doctrine\Common\Collections\ArrayCollection;
use Doctrine\Common\Collections\Collection;
use Doctrine\DBAL\Types\Types;
use Doctrine\ORM\Mapping as ORM;
use Symfony\Bridge\Doctrine\Types\UuidType;
use Symfony\Component\Uid\Uuid;

#[ORM\Entity]
#[ORM\Table(name: 'game')]
class Game
{
    #[ORM\Id]
    #[ORM\GeneratedValue]
    #[ORM\Column(type: Types::INTEGER)]
    private ?int $id = null;

    #[ORM\Column(type: UuidType::NAME, unique: true)]
    private ?Uuid $uuid = null;

    #[ORM\Column(type: Types::STRING, length: 10)]
    private ?string $playerSide = null; // 'white', 'black', 'random'

    #[ORM\Column(type: Types::STRING, length: 10)]
    private ?string $opponentType = null; // 'ai', 'hotseat'

    #[ORM\Column(type: Types::DATETIME_IMMUTABLE)]
    private ?\DateTimeImmutable $createdAt = null;

    #[ORM\Column(type: Types::DATETIME_IMMUTABLE, nullable: true)]
    private ?\DateTimeImmutable $gameOverAt = null;

    #[ORM\Column(type: Types::BOOLEAN)]
    private bool $whiteWins = false;

    #[ORM\Column(type: Types::BOOLEAN)]
    private bool $draw = false;

    #[ORM\OneToMany(targetEntity: Move::class, mappedBy: 'game', cascade: ['persist'], orphanRemoval: true)]
    #[ORM\OrderBy(['id' => 'ASC'])]
    private Collection $moves;

    public function __construct()
    {
        $this->uuid = Uuid::v4();
        $this->createdAt = new \DateTimeImmutable();
        $this->moves = new ArrayCollection();
    }

    public function getId(): ?int
    {
        return $this->id;
    }

    public function getUuid(): ?Uuid
    {
        return $this->uuid;
    }

    public function setUuid(Uuid $uuid): self
    {
        $this->uuid = $uuid;
        return $this;
    }

    public function getPlayerSide(): ?string
    {
        return $this->playerSide;
    }

    public function setPlayerSide(string $playerSide): self
    {
        $this->playerSide = $playerSide;
        return $this;
    }

    public function getOpponentType(): ?string
    {
        return $this->opponentType;
    }

    public function setOpponentType(string $opponentType): self
    {
        $this->opponentType = $opponentType;
        return $this;
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

    public function getGameOverAt(): ?\DateTimeImmutable
    {
        return $this->gameOverAt;
    }

    public function setGameOverAt(?\DateTimeImmutable $gameOverAt): self
    {
        $this->gameOverAt = $gameOverAt;
        return $this;
    }

    public function isWhiteWins(): bool
    {
        return $this->whiteWins;
    }

    public function setWhiteWins(bool $whiteWins): self
    {
        $this->whiteWins = $whiteWins;
        return $this;
    }

    public function isDraw(): bool
    {
        return $this->draw;
    }

    public function setDraw(bool $draw): self
    {
        $this->draw = $draw;
        return $this;
    }

    public function isGameOver(): bool
    {
        return $this->gameOverAt !== null;
    }

    /**
     * @return Collection<int, Move>
     */
    public function getMoves(): Collection
    {
        return $this->moves;
    }

    public function addMove(Move $move): self
    {
        if (!$this->moves->contains($move)) {
            $this->moves->add($move);
            $move->setGame($this);
        }

        return $this;
    }

    public function removeMove(Move $move): self
    {
        if ($this->moves->removeElement($move)) {
            // set the owning side to null (unless already changed)
            if ($move->getGame() === $this) {
                $move->setGame(null);
            }
        }

        return $this;
    }
}
