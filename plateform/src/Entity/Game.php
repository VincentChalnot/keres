<?php
declare(strict_types=1);

namespace App\Entity;

use App\Model\MovesData;
use App\Model\OpponentType;
use App\Repository\GameRepository;
use Doctrine\Common\Collections\ArrayCollection;
use Doctrine\Common\Collections\Collection;
use Doctrine\DBAL\Types\Types;
use Doctrine\ORM\Mapping as ORM;
use Symfony\Bridge\Doctrine\Types\UuidType;
use Symfony\Component\Uid\Uuid;

#[ORM\Entity(repositoryClass: GameRepository::class)]
#[ORM\Table(name: 'game')]
class Game
{
    #[ORM\Id]
    #[ORM\GeneratedValue]
    #[ORM\Column(type: Types::INTEGER)]
    private ?int $id = null;

    #[ORM\Column(type: UuidType::NAME, unique: true)]
    private readonly Uuid $uuid;

    #[ORM\Column(type: Types::INTEGER)]
    private int $opponentTypeValue;

    #[ORM\Column(type: Types::BOOLEAN)]
    private bool $isWhite;

    #[ORM\Column(type: Types::DATETIME_IMMUTABLE)]
    private \DateTimeImmutable $createdAt;

    #[ORM\Column(type: Types::DATETIME_IMMUTABLE, nullable: true)]
    private ?\DateTimeImmutable $gameOverAt = null;

    #[ORM\Column(type: Types::BOOLEAN)]
    private bool $whiteWins = false;

    #[ORM\Column(type: Types::BOOLEAN)]
    private bool $draw = false;

    #[ORM\Version]
    #[ORM\Column(type: Types::INTEGER)]
    private int $version = 1;

    #[ORM\Column(type: Types::DATETIME_IMMUTABLE, nullable: true)]
    private ?\DateTimeImmutable $deletedAt = null;

    /**
     * @var Collection<int, GameMove>
     */
    #[ORM\OneToMany(targetEntity: GameMove::class, mappedBy: 'game', cascade: [
        'persist',
        'remove',
    ], orphanRemoval: true)]
    #[ORM\OrderBy(['id' => 'ASC'])]
    private Collection $gameMoves;

    /**
     * @param OpponentType $opponentType
     * @param bool|null $isWhite If null, will be chosen randomly
     */
    public function __construct(OpponentType $opponentType = OpponentType::AI, ?bool $isWhite = null)
    {
        $this->uuid = Uuid::v4();
        $this->createdAt = new \DateTimeImmutable();
        $this->gameMoves = new ArrayCollection();
        $this->opponentTypeValue = $opponentType->value;
        if ($isWhite === null) {
            $this->isWhite = (bool) random_int(0, 1);
        } else {
            $this->isWhite = $isWhite;
        }
    }

    public function getId(): ?int
    {
        return $this->id;
    }

    public function getUuid(): Uuid
    {
        return $this->uuid;
    }

    public function isWhite(): bool
    {
        return $this->isWhite;
    }

    public function setIsWhite(bool $isWhite): self
    {
        $this->isWhite = $isWhite;

        return $this;
    }

    public function getOpponentType(): OpponentType
    {
        return OpponentType::from($this->opponentTypeValue);
    }

    public function setOpponentType(OpponentType $opponentType): self
    {
        $this->opponentTypeValue = $opponentType->value;

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

    public function getVersion(): int
    {
        return $this->version;
    }

    public function getDeletedAt(): ?\DateTimeImmutable
    {
        return $this->deletedAt;
    }

    public function setDeletedAt(?\DateTimeImmutable $deletedAt): self
    {
        $this->deletedAt = $deletedAt;

        return $this;
    }

    public function isDeleted(): bool
    {
        return $this->deletedAt !== null;
    }

    public function isGameOver(): bool
    {
        return $this->gameOverAt !== null;
    }

    /**
     * @return Collection<int, GameMove>
     */
    public function getGameMoves(): Collection
    {
        return $this->gameMoves;
    }

    public function isWhiteTurn(): bool
    {
        return $this->gameMoves->count() % 2 === 0;
    }

    public function addMove(Move $move): GameMove
    {
        $moveEntity = new GameMove($this, $move);
        $this->gameMoves->add($moveEntity);

        return $moveEntity;
    }

    public function getLastMoveAt(): ?\DateTimeImmutable
    {
        $lastMove = $this->gameMoves->last();
        if ($lastMove === false) {
            return null;
        }

        return $lastMove->getCreatedAt();
    }

    public function getMovesData(): MovesData
    {
        $data = new MovesData();
        foreach ($this->gameMoves as $moveEntity) {
            $data->addMove($moveEntity->getMove()->getMoveData());
        }

        return $data;
    }
}
