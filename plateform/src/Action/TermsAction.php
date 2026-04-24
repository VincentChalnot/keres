<?php
declare(strict_types=1);

namespace App\Action;

use Symfony\Component\HttpKernel\Attribute\AsController;
use Symfony\Component\Routing\Attribute\Route;

#[AsController]
class TermsAction
{
    #[Route(
        path: '/conditions-generales-de-vente',
        name: 'terms',
    )]
    public function __(): array
    {
        return [];
    }
}
