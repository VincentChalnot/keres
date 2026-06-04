<?php

declare(strict_types=1);

/** @noinspection PhpClassCanBeReadonlyInspection */

namespace App\Entity;

use App\Model\BoardData;
use Doctrine\DBAL\Types\Types;
use Doctrine\ORM\Mapping as ORM;

#[ORM\Entity(readOnly: true)]
#[ORM\Table(name: 'board_position')]
class BoardPosition
{
    #[ORM\Id]
    #[ORM\GeneratedValue]
    #[ORM\Column(type: Types::BIGINT)]
    private ?string $id;

    #[ORM\Column(type: Types::BINARY, length: 81, unique: true)]
    private readonly string $boardPositionData;

    public function __construct(BoardData $boardData)
    {
        $this->boardPositionData = $boardData->getPositionData();
    }

    public function getBoardPositionData(): string
    {
        return $this->boardPositionData;
    }
}
