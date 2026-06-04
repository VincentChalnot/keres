<?php
/** @noinspection PhpClassCanBeReadonlyInspection */
declare(strict_types=1);

namespace App\Entity;

use Doctrine\DBAL\Types\Types;
use Doctrine\ORM\Mapping as ORM;

#[ORM\Entity(readOnly: true)]
#[ORM\Table(name: 'game_move')]
class GameMove
{
    #[ORM\Id]
    #[ORM\GeneratedValue]
    #[ORM\Column(type: Types::BIGINT)]
    private ?string $id;

    #[ORM\Column(type: Types::DATETIME_IMMUTABLE)]
    private readonly \DateTimeImmutable $createdAt;

    #[ORM\ManyToOne(targetEntity: Game::class, inversedBy: 'gameMoves')]
    #[ORM\JoinColumn(nullable: false, onDelete: 'CASCADE')]
    private readonly Game $game;

    #[ORM\ManyToOne(targetEntity: Move::class, fetch: 'EAGER')]
    #[ORM\JoinColumn(nullable: false, onDelete: 'CASCADE')]
    private readonly Move $move;

    public function __construct(Game $game, Move $move)
    {
        $this->createdAt = new \DateTimeImmutable();
        $this->game = $game;
        $this->move = $move;
    }

    public function getCreatedAt(): ?\DateTimeImmutable
    {
        return $this->createdAt;
    }

    public function getMove(): Move
    {
        return $this->move;
    }

    public function getGame(): Game
    {
        return $this->game;
    }
}
