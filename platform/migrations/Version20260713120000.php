<?php

declare(strict_types=1);

namespace DoctrineMigrations;

use Doctrine\DBAL\Schema\Schema;
use Doctrine\Migrations\AbstractMigration;

final class Version20260713120000 extends AbstractMigration
{
    public function getDescription(): string
    {
        return 'Create feedback table for alpha tester feedback';
    }

    public function up(Schema $schema): void
    {
        $this->addSql('CREATE TABLE feedback (
            id UUID NOT NULL,
            user_id UUID NOT NULL,
            category VARCHAR(32) NOT NULL,
            message TEXT NOT NULL,
            created_at TIMESTAMP NOT NULL,
            PRIMARY KEY (id)
        )');
        $this->addSql('COMMENT ON COLUMN feedback.id IS \'(DC2Type:uuid)\'');
        $this->addSql('COMMENT ON COLUMN feedback.user_id IS \'(DC2Type:uuid)\'');
        $this->addSql('COMMENT ON COLUMN feedback.category IS \'(DC2Type:App\\Model\\FeedbackCategory)\'');
        $this->addSql('COMMENT ON COLUMN feedback.created_at IS \'(DC2Type:datetime_immutable)\'');
        $this->addSql('ALTER TABLE feedback ADD CONSTRAINT FK_D2913DE2A76ED395 FOREIGN KEY (user_id) REFERENCES "user" (id) NOT DEFERRABLE INITIALLY IMMEDIATE');
        $this->addSql('CREATE INDEX IDX_D2913DE2A76ED395 ON feedback (user_id)');
    }

    public function down(Schema $schema): void
    {
        $this->addSql('DROP TABLE feedback');
    }
}
