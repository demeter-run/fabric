# Account Sequence Diagram

The flow to create a new account will use the management API as RPC. The process needs to integrate the old Demeter database to be compatible and doesn't break the old Demeter version. By default, the account creation will create a new project that will be a namespace in the Kubernetes cluster. Therefore, the Kafka protocol must be used to send events to all clusters available on Demeter. All Demeter clusters will be connected in the Kafka protocol to create all resources required.

## Management RPC flow

```mermaid
sequenceDiagram   
    User->>+RPC_Driver: Create a new account
    RPC_Driver->>+Management_Domain: Call create account function

    Management_Domain->>+Demeter_Driven: Create an account in the old database(pg)
    Note left of Demeter_Driven: This flow replaces the logic <br/> of the old demeter API
    Demeter_Driven-->>-Management_Domain: All old business logic and integrations executed

    Management_Domain->>+State_Driven: Persist the new default project namespace
    State_Driven-->>-Management_Domain: State update confirmation 
    
    Management_Domain->>Event_Driven: Dispatch the event to create resource(namespace) in all cluster
    Note over Event_Driven: it will integrate with <br/> kafka protocol
    Event_Driven-->>Management_Domain: Event sent to a topic

    Management_Domain-->>-RPC_Driver: Account created
    RPC_Driver-->>-User: Account ready to use
```
