<?php
declare(strict_types=1);

namespace App\Action;

use App\Entity\Move as MoveEntity;
use App\Repository\GameRepository;
use Doctrine\ORM\EntityManagerInterface;
use Symfony\Bundle\FrameworkBundle\Controller\AbstractController;
use Symfony\Component\HttpFoundation\Request;
use Symfony\Component\HttpFoundation\Response;
use Symfony\Component\HttpKernel\Attribute\AsController;
use Symfony\Component\Routing\Attribute\Route;
use Symfony\Component\Uid\Uuid;
use Symfony\Contracts\HttpClient\HttpClientInterface;

#[AsController]
class SubmitMoveAction extends AbstractController
{
    public function __construct(
        private readonly GameRepository $gameRepository,
        private readonly EntityManagerInterface $entityManager,
        private readonly HttpClientInterface $httpClient,
    ) {
    }

    #[Route(
        path: '/play/{uuid}/move',
        name: 'submit_move',
        methods: ['POST'],
    )]
    public function __(string $uuid, Request $request): Response
    {
        $game = $this->gameRepository->findByUuid(Uuid::fromString($uuid));
        
        if (!$game) {
            return $this->json(['error' => 'Game not found'], Response::HTTP_NOT_FOUND);
        }

        if ($game->isGameOver()) {
            return $this->json(['error' => 'Game is already over'], Response::HTTP_BAD_REQUEST);
        }

        // Get the move as u16 from request body
        $content = $request->getContent();
        if (strlen($content) !== 2) {
            return $this->json(['error' => 'Invalid move data'], Response::HTTP_BAD_REQUEST);
        }

        $moveU16 = unpack('v', $content)[1];

        // Build the move list including all existing moves + the new one
        $existingMoves = $game->getMoves();
        $movesData = '';
        foreach ($existingMoves as $move) {
            $movesData .= $move->getMove();
        }
        $movesData .= $content; // Append the new move

        // Validate by calling the Rust backend
        try {
            /** @noinspection HttpUrlsUsage Internal service */
            $response = $this->httpClient->request(
                'POST',
                'http://backend:3000/replay-moves',
                [
                    'body' => $movesData,
                    'headers' => [
                        'Content-Type' => 'application/octet-stream',
                    ],
                ]
            );

            if ($response->getStatusCode() !== 200) {
                return $this->json(['error' => 'Invalid move'], Response::HTTP_BAD_REQUEST);
            }

            $boardData = $response->getContent();
            
            // The board data is 74 bytes: 9 u64s (72 bytes) + flags (2 bytes)
            // Extract game state flags from the last bytes
            $boardLength = strlen($boardData);
            if ($boardLength >= 74) {
                // Last 2 bytes contain flags
                $flagsData = substr($boardData, 72, 2);
                $flags = unpack('v', $flagsData)[1];
                
                // Extract flags (based on board encoding)
                // Bit 0: white_to_move
                // Bit 1: game_over
                // Bit 2: white_wins
                // Bit 3: draw
                $gameOver = (bool)(($flags >> 1) & 0x1);
                $whiteWins = (bool)(($flags >> 2) & 0x1);
                $draw = (bool)(($flags >> 3) & 0x1);

                // Save the move
                $moveEntity = new MoveEntity();
                $moveEntity->setMoveFromU16($moveU16);
                $moveEntity->setGame($game);
                $game->addMove($moveEntity);

                // Update game state if game is over
                if ($gameOver) {
                    $game->setGameOverAt(new \DateTimeImmutable());
                    $game->setWhiteWins($whiteWins);
                    $game->setDraw($draw);
                }

                $this->entityManager->persist($moveEntity);
                $this->entityManager->flush();

                // If game is in AI mode and not over, get AI move
                if ($game->getOpponentType() === 'ai' && !$gameOver) {
                    $aiMove = $this->getAiMove($boardData);
                    if ($aiMove !== null) {
                        // Validate and save AI move
                        $aiMovesData = $movesData . pack('v', $aiMove);
                        
                        $aiResponse = $this->httpClient->request(
                            'POST',
                            'http://backend:3000/replay-moves',
                            [
                                'body' => $aiMovesData,
                                'headers' => [
                                    'Content-Type' => 'application/octet-stream',
                                ],
                            ]
                        );

                        if ($aiResponse->getStatusCode() === 200) {
                            $boardData = $aiResponse->getContent();
                            
                            // Extract flags again for AI move result
                            if (strlen($boardData) >= 74) {
                                $flagsData = substr($boardData, 72, 2);
                                $flags = unpack('v', $flagsData)[1];
                                
                                $gameOver = (bool)(($flags >> 1) & 0x1);
                                $whiteWins = (bool)(($flags >> 2) & 0x1);
                                $draw = (bool)(($flags >> 3) & 0x1);

                                // Save AI move
                                $aiMoveEntity = new MoveEntity();
                                $aiMoveEntity->setMoveFromU16($aiMove);
                                $aiMoveEntity->setGame($game);
                                $game->addMove($aiMoveEntity);

                                // Update game state if game is over after AI move
                                if ($gameOver) {
                                    $game->setGameOverAt(new \DateTimeImmutable());
                                    $game->setWhiteWins($whiteWins);
                                    $game->setDraw($draw);
                                }

                                $this->entityManager->persist($aiMoveEntity);
                                $this->entityManager->flush();
                            }
                        }
                    }
                }

                // Return the board data and move information
                $movesData = [];
                foreach ($game->getMoves() as $moveEntity) {
                    $movesData[] = $moveEntity->getMoveAsU16();
                }
                
                return $this->json([
                    'success' => true,
                    'board' => base64_encode($boardData),
                    'moves' => $movesData,
                    'gameOver' => $gameOver,
                    'whiteWins' => $whiteWins,
                    'draw' => $draw,
                ]);
            }

            return $this->json(['error' => 'Invalid board data'], Response::HTTP_INTERNAL_SERVER_ERROR);

        } catch (\Exception $e) {
            return $this->json(['error' => 'Failed to validate move: ' . $e->getMessage()], Response::HTTP_INTERNAL_SERVER_ERROR);
        }
    }

    private function getAiMove(string $boardData): ?int
    {
        try {
            /** @noinspection HttpUrlsUsage Internal service */
            $response = $this->httpClient->request(
                'POST',
                'http://backend:3000/minimax-move',
                [
                    'body' => $boardData,
                    'headers' => [
                        'Content-Type' => 'application/octet-stream',
                    ],
                ]
            );

            if ($response->getStatusCode() === 200) {
                $moveData = $response->getContent();
                if (strlen($moveData) === 2) {
                    return unpack('v', $moveData)[1];
                }
            }
        } catch (\Exception $e) {
            // Log error but don't fail
            error_log('Failed to get AI move: ' . $e->getMessage());
        }

        return null;
    }
}
