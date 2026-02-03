<?php
declare(strict_types=1);

namespace App\Engine;

use App\Model\BoardData;
use App\Model\MovesData;
use Symfony\Contracts\HttpClient\HttpClientInterface;

readonly class EngineApi
{
    public function __construct(
        private HttpClientInterface $httpClient,
    ) {
    }

    public function replayMoves(MovesData $movesData): BoardData
    {
        $boardData = $this->callApi('replay-moves', $movesData->toBinary());

        return new BoardData($boardData);
    }

    private function callApi(string $endpoint, string $body): string
    {
        $apiResponse = $this->httpClient->request(
            'POST',
            "http://backend:3000/{$endpoint}",
            [
                'body' => $body,
                'headers' => [
                    'Content-Type' => 'application/octet-stream',
                ],
            ]
        );
        if ($apiResponse->getStatusCode() !== 200) {
            throw new \RuntimeException(
                "API call to {$endpoint} failed with status code ".$apiResponse->getStatusCode()
            );
        }

        return $apiResponse->getContent();
    }
}
