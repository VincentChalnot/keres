<?php

declare(strict_types=1);

namespace DoctrineMigrations;

use Doctrine\DBAL\Schema\Schema;
use Doctrine\Migrations\AbstractMigration;

final class Version20260325150001 extends AbstractMigration
{
    public function getDescription(): string
    {
        return 'Add deletedAt column to game table for soft delete';
    }

    public function up(Schema $schema): void
    {
        $this->addSql('ALTER TABLE game ADD deleted_at TIMESTAMP(0) WITHOUT TIME ZONE DEFAULT NULL');
        $this->addSql("COMMENT ON COLUMN game.deleted_at IS '(DC2Type:datetime_immutable)'");
    }

    public function down(Schema $schema): void
    {
        $this->addSql('ALTER TABLE game DROP deleted_at');
    }
}
