<?php
declare(strict_types=1);

namespace App\Command;

use App\Engine\BoardTreeManager;
use App\Engine\EngineApi;
use App\Engine\GameEngine;
use App\Repository\GameRepository;
use Symfony\Component\Console\Attribute\AsCommand;
use Symfony\Component\Console\Command\Command;
use Symfony\Component\Console\Input\InputArgument;
use Symfony\Component\Console\Input\InputInterface;
use Symfony\Component\Console\Output\OutputInterface;
use Symfony\Component\Uid\Uuid;

#[AsCommand(name: 'game:play-ai', description: 'Play AI move on a given game')]
class PlayAICommand extends Command
{
    public function __construct(
        private readonly GameRepository $gameRepository,
        private readonly GameEngine $gameEngine,
        ?string $name = null,
    ) {
        parent::__construct($name);
    }

    public function configure(): void
    {
        $this->addArgument('game-id', InputArgument::REQUIRED);
    }

    public function execute(InputInterface $input, OutputInterface $output): int
    {
        /** @var string $gameId */
        $gameId = $input->getArgument('game-id');
        $game = $this->gameRepository->findByUuid(Uuid::fromString($gameId));
        if (null === $game) {
            $output->writeln('<error>Game not found</error>');
            return 1;
        }

        $this->gameEngine->aiMove($game);

        return 0;
    }
}
