# Sequence Diagram

These diagrams show the flow of the processes into the architecture and how the flows will work.

## Account creation flow

An account will be persisted into the queue when the user call a creation. The RPC driver (management API) will persist a cache to manipulate and make query easly. And the daemon will create the resource in each cluster.

### RPC Driver

The RPC will call the domain to create the account where the domain will validate the information and integrate with the demeter legacy. When the account is created an event will be sent to the queue to handle the account creation.

```mermaid
sequenceDiagram
    actor User

    User->>+RPC_Driver: Create a new account
    RPC_Driver->>+Management_Domain: Call create account

    Management_Domain->>+Demeter_Driven: Create an account in the old database(pg)
    Note left of Demeter_Driven: This flow replaces the logic <br/> of the old demeter API
    Demeter_Driven-->>-Management_Domain: All old business logic and integrations executed

    Management_Domain->>Event_Driven: Submit an event to handle account

    Management_Domain-->>-RPC_Driver: Account created
    RPC_Driver-->>-User: Account created
```

### Event Driver

The event driver will be running togheter to the RPC driver watching the queue where it will handle account created.

```mermaid
sequenceDiagram
    actor Queue

    Queue->>+Event_Driver: Handle a new account
    Event_Driver->>+Management_Domain: Handle account function

    Management_Domain->>+Cache_Driven: Update cache
    Cache_Driven->>-Management_Domain: Cache updated

    Management_Domain->>-Event_Driver: Account handled
    Event_Driver->>-Queue: Ack event
```

### Daemon Driver

The Daemon Driver will be running in each cluster and watching the queue as well, but it will create the resource into the cluster.

```mermaid
sequenceDiagram
    actor Queue

    Queue->>+Fabric_Driver: Push event
    Fabric_Driver->>+Daemon_Domain: Call create namespace function

    Daemon_Domain->>+Cluster_Driven: Create resource into the cluster
    Note over Cluster_Driven: This function will integrate with <br/> the cluster and create the resource there
    Cluster_Driven-->>-Daemon_Domain: Confirmation resource created

    Daemon_Domain->>+Event_Driven: Dispatch the event to update the state
    Note over Event_Driven: Each cluster will dispatch the event and <br/> the state will be updated with each cluster <br/> that created the namespace
    Event_Driven-->>-Daemon_Domain: Event sent confirmation

    Daemon_Domain-->>-Fabric_Driver: Namespace created
    Fabric_Driver-->>-Queue: Ack the event
```

