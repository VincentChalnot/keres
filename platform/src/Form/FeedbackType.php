<?php

declare(strict_types=1);

namespace App\Form;

use App\Model\FeedbackCategory;
use Symfony\Component\Form\AbstractType;
use Symfony\Component\Form\Extension\Core\Type\ChoiceType;
use Symfony\Component\Form\Extension\Core\Type\SubmitType;
use Symfony\Component\Form\Extension\Core\Type\TextareaType;
use Symfony\Component\Form\FormBuilderInterface;
use Symfony\Component\OptionsResolver\OptionsResolver;
use Symfony\Component\Validator\Constraints\Length;
use Symfony\Component\Validator\Constraints\NotBlank;

class FeedbackType extends AbstractType
{
    public function buildForm(FormBuilderInterface $builder, array $options): void
    {
        $builder
            ->add('category', ChoiceType::class, [
                'label' => 'Category',
                'choices' => [
                    'Bug report' => FeedbackCategory::BUG,
                    'Suggestion' => FeedbackCategory::SUGGESTION,
                    'Gameplay feedback' => FeedbackCategory::GAMEPLAY,
                    'Other' => FeedbackCategory::OTHER,
                ],
                'constraints' => [
                    new NotBlank(message: 'Please select a category.'),
                ],
            ])
            ->add('message', TextareaType::class, [
                'label' => 'Your feedback',
                'constraints' => [
                    new NotBlank(message: 'Please enter your feedback.'),
                    new Length(min: 10, max: 5000, minMessage: 'Your feedback must be at least {{ limit }} characters.'),
                ],
                'attr' => [
                    'rows' => 6,
                    'placeholder' => 'Describe your feedback, bug, or suggestion...',
                ],
            ])
            ->add('submit', SubmitType::class, [
                'label' => 'Send feedback',
            ]);
    }

    public function configureOptions(OptionsResolver $resolver): void
    {
        $resolver->setDefaults([]);
    }
}
