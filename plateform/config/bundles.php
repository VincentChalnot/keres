<?php

return [
    Symfony\Bundle\FrameworkBundle\FrameworkBundle::class => ['all' => true],
    Sidus\TemplateBundle\SidusTemplateBundle::class => ['all' => true],
    Symfony\Bundle\TwigBundle\TwigBundle::class => ['all' => true],
    Pentatrion\ViteBundle\PentatrionViteBundle::class => ['all' => true],
    Doctrine\Bundle\DoctrineBundle\DoctrineBundle::class => ['all' => true],
    Doctrine\Bundle\MigrationsBundle\DoctrineMigrationsBundle::class => ['all' => true],
    Symfony\Bundle\MercureBundle\MercureBundle::class => ['all' => true],
    Symfony\Bundle\SecurityBundle\SecurityBundle::class => ['all' => true],
    Drenso\OidcBundle\DrensoOidcBundle::class => ['all' => true],
    Sidus\FilterBundle\SidusFilterBundle::class => ['all' => true],
    Sidus\DataGridBundle\SidusDataGridBundle::class => ['all' => true],
    Sidus\AdminBundle\SidusAdminBundle::class => ['all' => true],
    Symfony\Bundle\WebProfilerBundle\WebProfilerBundle::class => ['dev' => true, 'test' => true],
];
