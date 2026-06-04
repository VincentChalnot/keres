<?php
declare(strict_types=1);

namespace App\Action;

use Symfony\Component\HttpKernel\Attribute\AsController;
use Symfony\Component\Routing\Attribute\Route;

#[AsController]
class LegalNoticeAction
{
    #[Route(
        path: '/mentions-legales',
        name: 'legal_notice',
    )]
    public function __(): array
    {
        return [];
    }
}
