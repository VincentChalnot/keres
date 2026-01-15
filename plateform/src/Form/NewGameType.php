<?php
declare(strict_types=1);

namespace App\Form;

use Symfony\Component\Form\AbstractType;
use Symfony\Component\Form\Extension\Core\Type\ChoiceType;
use Symfony\Component\Form\Extension\Core\Type\SubmitType;
use Symfony\Component\Form\FormBuilderInterface;
use Symfony\Component\OptionsResolver\OptionsResolver;

class NewGameType extends AbstractType
{
    public function buildForm(FormBuilderInterface $builder, array $options): void
    {
        $builder
            ->add('playerSide', ChoiceType::class, [
                'label' => 'Side to play',
                'choices' => [
                    'White' => 'white',
                    'Black' => 'black',
                    'Random' => 'random',
                ],
                'expanded' => true,
                'data' => 'white', // Default selection
            ])
            ->add('opponentType', ChoiceType::class, [
                'label' => 'Opponent',
                'choices' => [
                    'AI' => 'ai',
                    'Hot-seat (2 players)' => 'hotseat',
                ],
                'expanded' => true,
                'data' => 'ai', // Default selection
            ])
            ->add('submit', SubmitType::class, [
                'label' => 'Start Game',
                'attr' => ['class' => 'button is-primary'],
            ])
        ;
    }

    public function configureOptions(OptionsResolver $resolver): void
    {
        $resolver->setDefaults([]);
    }
}
