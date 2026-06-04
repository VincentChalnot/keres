<?php

declare(strict_types=1);

namespace App\Security;

use Drenso\OidcBundle\OidcWellKnownParserInterface;

/**
 * Discord does not expose a userinfo_endpoint in its discovery doc.
 * This parser adds it manually.
 */
class DiscordWellKnownParser implements OidcWellKnownParserInterface
{
    /** @param array<string, mixed> $config */
    public function parseWellKnown(array $config): array
    {
        if (!isset($config['userinfo_endpoint'])) {
            $config['userinfo_endpoint'] = 'https://discord.com/api/users/@me';
        }

        return $config;
    }
}
