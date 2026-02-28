<?php

declare(strict_types=1);

namespace App\Action;

use App\Service\BoardRenderer;
use Symfony\Bundle\FrameworkBundle\Controller\AbstractController;
use Symfony\Component\HttpFoundation\Request;
use Symfony\Component\HttpFoundation\Response;
use Symfony\Component\HttpKernel\Attribute\AsController;
use Symfony\Component\Routing\Attribute\Route;

#[AsController]
class BoardImageAction extends AbstractController
{
    public function __construct(
        private readonly BoardRenderer $boardRenderer,
    ) {
    }

    #[Route(
        path: '/board/{boardData}.{_format}',
        name: 'board_image_format',
        requirements: ['_format' => 'svg|png|jpg|webp'],
    )]
    #[Route(
        path: '/board/{boardData}',
        name: 'board_image',
        defaults: ['_format' => 'svg'],
    )]
    public function __(string $boardData, string $_format, Request $request): Response
    {
        $binary = base64_decode(strtr($boardData, '-_', '+/'), true);

        if ($binary === false || \strlen($binary) !== 83) {
            throw $this->createNotFoundException('Invalid board data: expected 83 bytes base64url-encoded');
        }

        $flipped = $request->query->getBoolean('flipped');

        $svg = $this->boardRenderer->renderSvg($binary, $flipped);

        if ($_format === 'svg') {
            return new Response($svg, Response::HTTP_OK, [
                'Content-Type' => 'image/svg+xml',
                'Cache-Control' => 'public, max-age=31536000, immutable',
            ]);
        }

        $contentTypes = [
            'png' => 'image/png',
            'jpg' => 'image/jpeg',
            'webp' => 'image/webp',
        ];

        $raster = $this->boardRenderer->convertToRaster($svg, $_format);

        return new Response($raster, Response::HTTP_OK, [
            'Content-Type' => $contentTypes[$_format],
            'Cache-Control' => 'public, max-age=31536000, immutable',
        ]);
    }
}
