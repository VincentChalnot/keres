<?php

$finder = (new PhpCsFixer\Finder())
    ->in(__DIR__)
    ->exclude('var')
    ->exclude('vendor')
    ->exclude('node_modules')
    ->notPath([
        'config/bundles.php',
        'config/reference.php',
    ])
;

return (new PhpCsFixer\Config())
    ->setRiskyAllowed(true)
    ->setRules([
        '@Symfony' => true,
        '@Symfony:risky' => true,
        'declare_strict_types' => true,
        'array_syntax' => ['syntax' => 'short'],
        'trailing_comma_in_multiline' => true,
        'yoda_style' => true,
        'void_return' => true,
        'no_unset_on_property' => true,
        'no_superfluous_elseif' => true,
        'nullable_type_declaration_for_default_null_value' => true,
        'combine_consecutive_issets' => true,
        'combine_consecutive_unsets' => true,
        'date_time_immutable' => true,
        'phpdoc_to_property_type' => true,
        'phpdoc_align' => ['align' => 'left'],
        'phpdoc_types_order' => ['null_adjustment' => 'always_last', 'sort_algorithm' => 'alpha'],
        'general_phpdoc_annotation_remove' => ['annotations' => ['author', 'version', 'since', 'package', 'subpackage']],
        'blank_line_before_statement' => [
            'statements' => ['return', 'throw', 'if', 'foreach', 'for', 'while', 'try', 'do', 'switch'],
        ],
        'ordered_class_elements' => [
            'order' => [
                'use_trait',
                'constant_public', 'constant_protected', 'constant_private',
                'property_public', 'property_protected', 'property_private',
                'construct', 'destruct', 'magic', 'phpunit',
                'method_public', 'method_protected', 'method_private',
            ],
            'sort_algorithm' => 'none',
        ],
    ])
    ->setFinder($finder)
    ;
