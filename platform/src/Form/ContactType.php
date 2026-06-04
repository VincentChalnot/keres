<?php
declare(strict_types=1);

namespace App\Form;

use Symfony\Component\Form\AbstractType;
use Symfony\Component\Form\Extension\Core\Type\EmailType;
use Symfony\Component\Form\Extension\Core\Type\SubmitType;
use Symfony\Component\Form\Extension\Core\Type\TextareaType;
use Symfony\Component\Form\Extension\Core\Type\TextType;
use Symfony\Component\Form\FormBuilderInterface;
use Symfony\Component\OptionsResolver\OptionsResolver;
use Symfony\Component\Validator\Constraints\Email;
use Symfony\Component\Validator\Constraints\Length;
use Symfony\Component\Validator\Constraints\NotBlank;

class ContactType extends AbstractType
{
    public function buildForm(FormBuilderInterface $builder, array $options): void
    {
        $builder
            ->add('name', TextType::class, [
                'label' => 'Votre nom',
                'constraints' => [
                    new NotBlank(message: 'Veuillez indiquer votre nom.'),
                    new Length(max: 100),
                ],
                'attr' => ['placeholder' => 'Votre nom complet'],
            ])
            ->add('email', EmailType::class, [
                'label' => 'Votre adresse e-mail',
                'constraints' => [
                    new NotBlank(message: 'Veuillez indiquer votre adresse e-mail.'),
                    new Email(message: 'Cette adresse e-mail n\'est pas valide.'),
                ],
                'attr' => ['placeholder' => 'exemple@domaine.fr'],
            ])
            ->add('subject', TextType::class, [
                'label' => 'Sujet',
                'constraints' => [
                    new NotBlank(message: 'Veuillez indiquer un sujet.'),
                    new Length(max: 200),
                ],
                'attr' => ['placeholder' => 'Objet de votre message'],
            ])
            ->add('message', TextareaType::class, [
                'label' => 'Message',
                'constraints' => [
                    new NotBlank(message: 'Veuillez saisir un message.'),
                    new Length(min: 10, max: 2000, minMessage: 'Votre message doit contenir au moins {{ limit }} caractères.'),
                ],
                'attr' => [
                    'rows' => 6,
                    'placeholder' => 'Votre message...',
                ],
            ])
            ->add('submit', SubmitType::class, [
                'label' => 'Envoyer',
            ]);
    }

    public function configureOptions(OptionsResolver $resolver): void
    {
        $resolver->setDefaults([]);
    }
}
