<?php

declare(strict_types=1);

namespace App\Action;

use Symfony\Component\HttpKernel\Attribute\AsController;
use Symfony\Component\Routing\Attribute\Route;

#[AsController]
class IndexAction
{
    #[Route(
        path: '/',
        name: 'index',
    )]
    public function __(): array
    {
        return [];
    }
}
