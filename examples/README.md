# How to run?

Two binaries need to be executed, `rpc` which will allow external requests to create resources, like projects and ports and `daemon` which needs to be executed in each cluster to handle the events and create the resources on Kubernetes.

## rpc

It's possible to create a toml file as config and set the file path in the env `RPC_CONFIG`, but it's possible to set the config using the prefix `RPC_`

```
addr="0.0.0.0:5000"
db_path="dev.db"
brokers="localhost:19092"
```

## daemon

It's possible to create a toml file as config and set the file path in the env `DAEMON_CONFIG`, but it's possible to set the config using the prefix `DAEMON_`

```
brokers="localhost:19092"
```
