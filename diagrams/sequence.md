# Sequence Diagram

These diagrams show the flow of the processes into the architecture and how the flows will work.

## User creation flow

### RPC Driver

The user will first authenticate in OAuth and get an Access token. Then, the user will request the fabric to create the user or return the existing user.

```mermaid
sequenceDiagram
    actor User

    User->>+OAuth: Login/Signup in oauth
    OAuth->>-User: Access Token

    User->>+RPC: Send command to create user
    RPC->>+Management_Domain: Create user

    Management_Domain->>+OAuth_Driven: Verify access token
    alt Invalid token
        OAuth_Driven->>Management_Domain: Invalid access token
        Management_Domain->>RPC: Invalid access token
        RPC->>User: Invalid access token
    end
    OAuth_Driven->>-Management_Domain: Return user id

    Management_Domain->>+Cache_Driven: Get user
    alt User already exists
        Cache_Driven->>Management_Domain: Return user
        Management_Domain->>RPC: User already exists
        RPC->>User: Return the user
    end
    Cache_Driven->>-Management_Domain: User doesn't exist

    Management_Domain->>+OAuth_Driven: Get user profile
    OAuth_Driven->>-Management_Domain: Return user profile

    Management_Domain->>+Event_Driven: Send event user created
    Event_Driven->>-Management_Domain: Return confirmation

    Management_Domain->>-RPC: User created
    RPC->>-User: Return the user
```

### Event Driver

If all is ok, an event of the user created will be sent to a queue to persist that user. The event drive will be listening for events and will persist the user in the cache.

```mermaid
sequenceDiagram
    Queue->>+Event_Driver: New event: User Created

    Event_Driver->>+Management_Domain: Insert new user in the cache

    Management_Domain->>+Cache_Driven: Get user
    alt User already exists
        Cache_Driven->>Management_Domain: Return User
        Management_Domain->>Event_Driver: User already exists
        Event_Driver->>Queue: Ack event
    end

    Cache_Driven->>-Management_Domain: User doesn't exist
    Management_Domain->>+Cache_Driven: Insert new user
    Cache_Driven->>-Management_Domain: Return Ok

    Management_Domain->>-Event_Driver: User inserted
    Event_Driver->>-Queue: Ack event
```
