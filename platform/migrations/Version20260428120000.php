<?php

declare(strict_types=1);

namespace DoctrineMigrations;

use Doctrine\DBAL\Schema\Schema;
use Doctrine\Migrations\AbstractMigration;

final class Version20260428120000 extends AbstractMigration
{
    public function getDescription(): string
    {
        return 'Split user auth credentials to separate table';
    }

    public function up(Schema $schema): void
    {
        $this->addSql('CREATE TABLE user_auth (
            id UUID NOT NULL,
            user_id UUID NOT NULL,
            provider VARCHAR(32) NOT NULL,
            provider_id VARCHAR(255) NOT NULL,
            created_at TIMESTAMP NOT NULL,
            PRIMARY KEY (id)
        )');
        $this->addSql('CREATE UNIQUE INDEX uniq_user_auth_provider ON user_auth (user_id, provider)');
        $this->addSql('COMMENT ON COLUMN user_auth.id IS \'(DC2Type:uuid)\'');
        $this->addSql('COMMENT ON COLUMN user_auth.user_id IS \'(DC2Type:uuid)\'');
        $this->addSql('COMMENT ON COLUMN user_auth.created_at IS \'(DC2Type:datetime_immutable)\'');
        $this->addSql('ALTER TABLE user_auth ADD CONSTRAINT FK_E2E7A62C7E3C61F9 FOREIGN KEY (user_id) REFERENCES "user" (id) NOT DEFERRABLE INITIALLY IMMEDIATE');

        $this->addSql('INSERT INTO user_auth (id, user_id, provider, provider_id, created_at)
            SELECT gen_random_uuid(), id, provider, provider_id, created_at FROM "user"');

        $this->addSql('ALTER TABLE "user" DROP provider');
        $this->addSql('ALTER TABLE "user" DROP provider_id');
    }

    public function down(Schema $schema): void
    {
        $this->addSql('ALTER TABLE "user" ADD provider VARCHAR(32) NOT NULL');
        $this->addSql('ALTER TABLE "user" ADD provider_id VARCHAR(255) NOT NULL');

        $this->addSql('UPDATE "user" SET provider = ua.provider, provider_id = ua.provider_id
            FROM user_auth ua WHERE "user".id = ua.user_id');

        $this->addSql('DROP TABLE user_auth');
    }
}
