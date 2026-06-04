<?php

declare(strict_types=1);

namespace App\Model;

use Doctrine\Common\Collections\ArrayCollection;
use Doctrine\Common\Collections\Collection;

class MovesData
{
    /* @var Collection<int, MoveData> */
    private Collection $moves;

    public function __construct()
    {
        $this->moves = new ArrayCollection();
    }

    /**
     * @return Collection<int, MoveData>
     */
    public function getMoves(): Collection
    {
        return $this->moves;
    }

    public function addMove(MoveData $moveData): self
    {
        $this->moves[] = $moveData;

        return $this;
    }

    public function toBinary(): string
    {
        $data = '';

        foreach ($this->moves as $move) {
            $data .= $move->data;
        }

        return $data;
    }
}
