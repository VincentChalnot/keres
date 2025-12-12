<?php
declare(strict_types=1);

namespace App\Action;

use Symfony\Component\HttpKernel\Attribute\AsController;
use Symfony\Component\Routing\Attribute\Route;

#[AsController]
class PlayAction
{
    #[Route(
        path: '/{uid}?',
        name: 'play',
    )]
    public function __(): array
    {
        return [];
    }
}
