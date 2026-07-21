<?php

declare(strict_types=1);

namespace App\Action;

use Symfony\Component\HttpKernel\Attribute\AsController;

/**
 * Route target for the dev-only /dev/login endpoint.
 *
 * No #[Route] attribute here on purpose: the route is registered explicitly
 * in config/routes/dev/dev_login.yaml, which is imported only when
 * kernel.environment is "dev". The request never actually reaches this
 * method — App\Security\DevLoginAuthenticator intercepts and authenticates
 * before the controller runs, exactly like LoginAction::check() does for
 * the real OIDC callback.
 */
#[AsController]
class DevLoginAction
{
    public function check(): never
    {
        throw new \LogicException('This should be handled by DevLoginAuthenticator.');
    }
}
