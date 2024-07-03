# Organization and Payments

## Context

Currently, when a user creates a new account, an organization and a project are created by default. The organization will be used to link all projects and payment settings.

## Decision

- Organization

  The organization will be degraded but the fabric will have support for it in legacy driven just for compatibility.

- Payments

  Strip integration will happen just when the user wants to upgrade the plan(tier), so the user will be redirected to a screen to set the payment and the strip will be linked to the project. In fabric, the Strip integration will be into the payment driven where in the future new payment methods can be offered.
