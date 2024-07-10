# Organization and Payments

## Context

In the current Demeter platform, it is necessary to create an organization to link projects and users.

## Decision

- Organization

The organization will be deprecated but the fabric will have support for it in legacy driven just for compatibility.

- Payments

Stripe integration will happen just when the user wants to upgrade the plan(tier), so the user will be redirected to a screen to set the payment and the stripe will be linked to the project. In fabric, the Stripe integration will be into the payment driven where in the future new payment methods can be offered.
