<?php

declare(strict_types=1);

namespace App\Action;

use Symfony\Component\HttpKernel\Attribute\AsController;
use Symfony\Component\Routing\Attribute\Route;

#[AsController]
class RulesAction
{
    #[Route(
        path: '/rules',
        name: 'rules',
    )]
    public function __(): array
    {
        return [];
    }
}
