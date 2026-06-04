<?php

declare(strict_types=1);

namespace DoctrineMigrations;

use Doctrine\DBAL\Schema\Schema;
use Doctrine\Migrations\AbstractMigration;

/**
 * Add version column to game table for optimistic locking.
 */
final class Version20260325150000 extends AbstractMigration
{
    public function getDescription(): string
    {
        return 'Add version column to game table for optimistic locking';
    }

    public function up(Schema $schema): void
    {
        $this->addSql('ALTER TABLE game ADD version INT NOT NULL DEFAULT 1');
    }

    public function down(Schema $schema): void
    {
        $this->addSql('ALTER TABLE game DROP COLUMN version');
    }
}
