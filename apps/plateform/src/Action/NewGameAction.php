<?php
declare(strict_types=1);

namespace App\Action;

use Symfony\Component\HttpFoundation\RedirectResponse;
use Symfony\Component\HttpKernel\Attribute\AsController;
use Symfony\Component\Routing\Attribute\Route;

#[AsController]
class NewGameAction
{
    #[Route(
        path: '/play',
        name: 'new_game',
    )]
    public function __(): RedirectResponse
    {
        return new RedirectResponse('/play/'.uniqid());
    }
}
