# Payment Gateway and Legacy Organization

## Context

When the user creates a new account, an organization and project are created by default. An integration is made with Strip to create a customer where it will be related to the organization. But the organization is not necessary anymore and each project can have a Strip.

## Decision

- Organization
  The organization will be degraded but the fabric will have support for it in legacy driven just for compatibility.

- Strip Payments
  Strip integration will happen just when the user wants to upgrade the plan(tier), so the user will be redirected to a screen to set the payment and the strip will be linked to the project. In fabric, the Strip integration will be into the payment driven where in the future new payment methods can be offered.
