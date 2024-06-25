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

        Management_Domain->>+Legacy_Driven: Execute legacy account creation flow
        Legacy_Driven->>+Auth0: Verify user token
        
        alt Invalid jwt token
            Auth0-->>Legacy_Driven: Jwt invalid
            Legacy_Driven-->>Management_Domain: Invalid auth token
            Management_Domain-->>RPC_Driver: Account isn't able to be created
            RPC_Driver-->>User: Fail to create account        
        end 

        Auth0-->>-Legacy_Driven: Return the user id
        Legacy_Driven->>+Postgres: Get user

        alt User already exist
            Postgres-->>Legacy_Driven: Return user
            Legacy_Driven-->>Management_Domain: User already exist
            Management_Domain-->>RPC_Driver: User already exist
            RPC_Driver-->>User: User already exist
        end
        
        Postgres-->>-Legacy_Driven: User not exist
        Legacy_Driven->>+Auth0: Get user profile
        Auth0-->>-Legacy_Driven: Return user auth profile

        Legacy_Driven->>+Postgres: Create user
        Postgres-->>-Legacy_Driven: Return user id

        Legacy_Driven->>+Stripe: Create strip customer
        Stripe-->>-Legacy_Driven: Return customer

        Legacy_Driven->>+Postgres: Create organization
        Postgres-->>-Legacy_Driven: Return organization id
    end
    
    Management_Domain->>Management_Domain: Create default project

    Legacy_Driven-->>-Management_Domain: Legacy integration executed
    Management_Domain-->>-RPC_Driver: Account created
    RPC_Driver-->>-User: Account created
```
