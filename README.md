# Fabric

Fabric will allow Demeter to decentralise clusters. Each cluster will run a monitor receiving events from a queue process the event into the cluster and create Kubernetes resources. Still, the cluster can configure which resources will be available. One cluster will be the central demeter cluster where the RPC will be running to accept creation events and make a cache of all resources created.

## Contributing

There are two binaries available, RPC and daemon. The RPC is responsible for validating requests and creating events. The daemon is responsible for creating resources in the Kubernetes, integrating directly with the cluster. To run both binaries is necessary a kafka service with the topic `events` created, so the `docker-compose` file needs to be executed to run the kafka.

### Cache System

The cache system is using SQLite, so it's necessary to install `sqlx` cli to create the database and execute the migrations. If there are updates on the tables, the cli needs to be executed again to update the .sqlx map files.

To install the sqlx cli, use the cargo install

```sh
 cargo install sqlx-cli
```

Follow the command below to prepare the environment to run

```sh
# Set db path env
export DATABASE_URL="sqlite:dev.db"

# Create database
cargo sqlx db create

# Start migrations
cargo sqlx migrate run --source ./src/driven/cache/migrations
```

If there are updates in the schemas, execute the command below to update the sqlx map files

```sh
cargo sqlx prepare
```

### Queue dependence

Start the dependences with the command below

```sh
docker compose up -d
```

### Run binaries

Command to run RPC

```sh
cargo run --bin=rpc
```

Command to run Daemon

```sh
cargo run --bin=daemon
```

## Test

To run the tests, execute the command below

```sh
cargo test --lib
```
