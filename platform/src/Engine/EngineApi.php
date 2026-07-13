<?php

declare(strict_types=1);

namespace App\Engine;

use App\Model\BoardData;
use App\Model\MoveData;
use App\Model\MovesData;
use Symfony\Contracts\HttpClient\HttpClientInterface;

readonly class EngineApi
{
    public function __construct(
        private HttpClientInterface $httpClient,
        private string $backendApiUrl,
        private string $aiBackendApiUrl,
    ) {
    }

    public function replayMoves(MovesData $movesData): BoardData
    {
        $boardData = $this->callApi('replay-moves', $movesData->toBinary());

        return new BoardData($boardData);
    }

    public function aiMove(MovesData $movesData): MoveData
    {
        try {
            $moveData = $this->callApi('engine-move-game', $movesData->toBinary(), $this->aiBackendApiUrl);
        } catch (\RuntimeException) {
            $moveData = $this->callApi('engine-move-game', $movesData->toBinary());
        }

        return new MoveData($moveData);
    }

    private function callApi(string $endpoint, string $body, ?string $baseUrl = null): string
    {
        $url = rtrim($baseUrl ?? $this->backendApiUrl, '/').'/'.ltrim($endpoint, '/');
        $apiResponse = $this->httpClient->request(
            'POST',
            $url,
            [
                'body' => $body,
                'headers' => [
                    'Content-Type' => 'application/octet-stream',
                ],
            ]
        );

        if (200 !== $apiResponse->getStatusCode()) {
            throw new \RuntimeException("API call to {$endpoint} failed with status code ".$apiResponse->getStatusCode());
        }

        return $apiResponse->getContent();
    }
}
