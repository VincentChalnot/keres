<?php

declare(strict_types=1);

namespace App\Action\Api;

use Symfony\Component\HttpFoundation\Request;
use Symfony\Component\HttpFoundation\Response;
use Symfony\Component\HttpKernel\Attribute\AsController;
use Symfony\Component\Routing\Attribute\Route;

#[AsController]
readonly class MovesAction extends AbstractForwardToApiAction
{
    #[Route(
        path: '/api/moves',
        name: 'api_moves',
        methods: ['POST'],
    )]
    public function __(Request $request): Response
    {
        return $this->forward($request);
    }
}
