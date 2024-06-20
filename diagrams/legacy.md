# Legacy Sequence Diagram

The new Demeter decentralized ecosystem needs to have support for the legacy, and some diagrams with legacy integrations are here to describe these flows.

## Account creation flow

The account creation flow needs to integrate with the legacy Demeter ecosystem, therefore this Diagram describes the flow to integrate.

```mermaid
sequenceDiagram
    actor User

    User->>+RPC_Driver: Create a new account
    RPC_Driver->>+Management_Domain: Call account creation function

    critical Legacy integrations

        Management_Domain->>Management_Domain: Validate payload

        Management_Domain->>+Demeter_Driven: Execute legacy account creation flow
        Demeter_Driven->>+Auth0: Verify user token

        alt Invalid jwt token
            Auth0-->>Demeter_Driven: Jwt invalid
            Demeter_Driven-->>Management_Domain: Invalid auth token
            Management_Domain-->>RPC_Driver: Account isn't able to be created
            RPC_Driver-->>User: Fail to create account
        end

        Auth0-->>-Demeter_Driven: Return the user id
        Demeter_Driven->>+Postgres: Get user

        alt User already exist
            Postgres-->>Demeter_Driven: Return user
            Demeter_Driven-->>Management_Domain: User already exist
            Management_Domain-->>RPC_Driver: User already exist
            RPC_Driver-->>User: User already exist
        end

        Postgres-->>-Demeter_Driven: User not exist
        Demeter_Driven->>+Auth0: Get user profile
        Auth0-->>-Demeter_Driven: Return user auth profile

        Demeter_Driven->>+Postgres: Create user
        Postgres-->>-Demeter_Driven: Return user id

        Demeter_Driven->>+Stripe: Create strip customer
        Stripe-->>-Demeter_Driven: Return customer

        Demeter_Driven->>+Postgres: Create organization
        Postgres-->>-Demeter_Driven: Return organization id

        Management_Domain->>Management_Domain: Create default project
    end

    Demeter_Driven-->>-Management_Domain: Legacy integration executed
    Management_Domain->>Event_Driven: Submit an event to handle account
    Management_Domain-->>-RPC_Driver: Account created
    RPC_Driver-->>-User: Account created
```
