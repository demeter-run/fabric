# Stripe

## Context

The way that the user can configure the payment method.

## Decision

- Payment Setup

Payment is associated to a project. One payment method per project. Stripe Intent will be defined in the user interface

- Flows

Sequence diagrams need to be created for each flow.

```
 ACCOUNT
- A new project will be created by default
* account creation

PROJECT
- A default Stripe customer is created when the user create a project by default
* project creation
* project deletion
* project API key creation
* project API key deletion

PORT
* port creation
* port deletion
* port usage and details

TIERS
* tiers upgrade
* tiers downgrade

PAYMENTS
- Payment is associated to a project
- One payment method per project
- Stripe Intent will be defined in the user interface
* payment method updating
* payment method deletion
* payment transactions Webhook (stripe/ada)

USERS
* users invitation to a project
* users deletion from a project
```
