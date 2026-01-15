<?php
declare(strict_types=1);

namespace App\Action\Api;

use Symfony\Component\HttpFoundation\Request;
use Symfony\Component\HttpFoundation\Response;
use Symfony\Component\HttpKernel\Attribute\AsController;
use Symfony\Component\Routing\Attribute\Route;
use Symfony\Contracts\HttpClient\HttpClientInterface;

readonly abstract class AbstractForwardToApiAction
{
    public function __construct(
        private HttpClientInterface $httpClient,
    ) {
    }

    protected function forward(Request $request): Response
    {
        // Remove the '/api' prefix using regexp to forward to the backend service
        $requestUri = preg_replace('#^/api#', '', $request->getRequestUri());

        /** @noinspection HttpUrlsUsage Internal service */
        $apiResponse = $this->httpClient->request(
            $request->getMethod(),
            "http://backend:3000{$requestUri}",
            [
                'body' => $request->getContent(),
                'headers' => [
                    'Content-Type' => 'application/octet-stream',
                ],
            ]
        );

        // Forward the response from the backend service
        return new Response(
            $apiResponse->getContent(),
            $apiResponse->getStatusCode(),
            $apiResponse->getHeaders(false)
        );
    }
}
