<?php

declare(strict_types=1);

namespace App\Model;

enum FeedbackCategory: string
{
    case BUG = 'bug';
    case SUGGESTION = 'suggestion';
    case GAMEPLAY = 'gameplay';
    case OTHER = 'other';
}
