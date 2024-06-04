# Sequence Diagram

These diagrams show the flow of the processes into the architecture and how the flows will work.

## RPC account creation flow

The flow to create a new account will use the management API as RPC. The process needs to integrate the old Demeter database to be compatible and doesn't break the old Demeter version. By default, the account creation will create a new project that will be a namespace in the Kubernetes cluster. Therefore, the Kafka protocol must be used to send events to all clusters available on Demeter. All Demeter clusters will be connected in the Kafka protocol to create all resources required.

```mermaid
sequenceDiagram
    actor User
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

## RPC project creation flow

When the user needs to create a new project, the user will make an RPC to the management service that will create the project in the state and will send an event to create a project namespace in all clusters.

```mermaid
sequenceDiagram
    actor User

    User->>+RPC_Driver: Create a new project
    RPC_Driver->>+Management_Domain: Call create project function

    Management_Domain->>+Demeter_Driven: Create an project in the old database(pg)
    Note over Demeter_Driven: This flow replaces the logic <br/> of the old demeter API
    Demeter_Driven-->>-Management_Domain: All old business logic and integrations executed

    Management_Domain->>+State_Driven: Persist the new project namespace
    State_Driven-->>-Management_Domain: State update confirmation

    Management_Domain->>Event_Driven: Dispatch the event to create resource(namespace) in all cluster
    Note over Event_Driven: it will integrate with <br/> kafka protocol
    Event_Driven-->>Management_Domain: Event sent to a topic

    Management_Domain-->>-RPC_Driver: Project created
    RPC_Driver-->>-User: Project ready to use

```

## Fabric resource creation flow

Each cluster will follow this flow to create resources, e.g. namespace and ports.

```mermaid
sequenceDiagram
    actor Kafka

    Kafka->>+Fabric_Driver: Push events
    Fabric_Driver->>+Daemon_Domain: Call create namespace function

    Daemon_Domain->>+Cluster_Driven: Create resource into the cluster
    Note over Cluster_Driven: This function will integrate with <br/> the cluster and create the resource there
    Cluster_Driven-->>-Daemon_Domain: Confirmation resource created

    Daemon_Domain->>+Event_Driven: Dispatch the event to update the state
    Note over Event_Driven: Each cluster will dispatch the event and <br/> the state will be updated with each cluster <br/> that created the namespace
    Event_Driven-->>-Daemon_Domain: Event sent to a topic

    Daemon_Domain-->>-Fabric_Driver: Namespace created
    Fabric_Driver-->>-Kafka: Ack the event
```
