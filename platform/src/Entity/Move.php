<?php

declare(strict_types=1);

/** @noinspection PhpClassCanBeReadonlyInspection */

namespace App\Entity;

use App\Model\MoveData;
use Doctrine\DBAL\Types\Types;
use Doctrine\ORM\Mapping as ORM;

#[ORM\Entity(readOnly: true)]
#[ORM\Table(name: 'move')]
#[ORM\UniqueConstraint(name: 'move_unique_idx', fields: ['moveData', 'fromBoardPosition'])]
class Move
{
    #[ORM\Id]
    #[ORM\GeneratedValue]
    #[ORM\Column(type: Types::BIGINT)]
    private ?string $id;

    #[ORM\Column(type: Types::BINARY, length: 2)]
    private readonly string $moveData;

    #[ORM\ManyToOne(targetEntity: BoardPosition::class)]
    #[ORM\JoinColumn(nullable: false)]
    private readonly BoardPosition $fromBoardPosition;

    #[ORM\ManyToOne(targetEntity: BoardPosition::class)]
    #[ORM\JoinColumn(nullable: false)]
    private readonly BoardPosition $toBoardPosition;

    public function __construct(MoveData $moveData, BoardPosition $fromBoardPosition, BoardPosition $toBoardPosition)
    {
        $this->moveData = $moveData->data;
        $this->fromBoardPosition = $fromBoardPosition;
        $this->toBoardPosition = $toBoardPosition;
    }

    public function getMoveData(): MoveData
    {
        return new MoveData($this->moveData);
    }

    public function getFromBoardPosition(): BoardPosition
    {
        return $this->fromBoardPosition;
    }

    public function getToBoardPosition(): BoardPosition
    {
        return $this->toBoardPosition;
    }
}
