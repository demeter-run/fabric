# How to run?

Two binaries need to be executed, `rpc` which will allow external requests to create resources, like projects and ports and `daemon` which needs to be executed in each cluster to handle the events and create the resources on Kubernetes.

## rpc

It's possible to create a toml file as config and set the file path in the env `RPC_CONFIG`, but it's possible to set the config using the prefix `RPC_`. The auth url is the endpoint to integrate with auth0.

Use the [rpc config](config/rpc.toml)

## daemon

It's possible to create a toml file as config and set the file path in the env `DAEMON_CONFIG`, but it's possible to set the config using the prefix `DAEMON_`

Use the [daemon config](config/daemon.toml)

## cli 

Use the [cli config](config/cli.toml)
