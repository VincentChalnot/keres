<?php

declare(strict_types=1);

namespace DoctrineMigrations;

use Doctrine\DBAL\Schema\Schema;
use Doctrine\Migrations\AbstractMigration;

final class Version20260327163301 extends AbstractMigration
{
    public function getDescription(): string
    {
        return 'Add nullable owner relation on game table';
    }

    public function up(Schema $schema): void
    {
        $this->addSql('ALTER TABLE game ADD owner_id UUID DEFAULT NULL');
        $this->addSql('COMMENT ON COLUMN game.owner_id IS \'(DC2Type:uuid)\'');
        $this->addSql('ALTER TABLE game ADD CONSTRAINT FK_232B318C7E3C61F9 FOREIGN KEY (owner_id) REFERENCES "user" (id) NOT DEFERRABLE INITIALLY IMMEDIATE');
        $this->addSql('CREATE INDEX IDX_232B318C7E3C61F9 ON game (owner_id)');
    }

    public function down(Schema $schema): void
    {
        $this->addSql('ALTER TABLE game DROP CONSTRAINT FK_232B318C7E3C61F9');
        $this->addSql('DROP INDEX IDX_232B318C7E3C61F9');
        $this->addSql('ALTER TABLE game DROP owner_id');
    }
}
