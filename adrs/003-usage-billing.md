# Usage and billing

## Context

Each cluster needs to send the usage metrics of each user. Then, the main cluster will aggregate the data and make the invoice, integrating it with the payment gateway.

## Decision

Each cluster will be running a usage driver to collect the usage data and send them as an event to the main cluster through the queue. In the main cluster, an event driver will be running and it will capture the event and persist in the cache db. The invoice will be triggered by another driver(billing) and it will be executed once a month, the payment gateway needs to be called to generate the invoice.

- Usage  
  This needs to be executed in each cluster and triggered once an hour, integrating with Prometheus to collect metrics of data usage and send an event to the queue.

- Billing  
  The billing will be executed once a month in the main cluster, getting user usage in the cache of all clusters, and integrating with a payment gateway to make the invoice.

## Rules

- Usage driver(daemon)
  - each cluster will run this driver and send report usage on Kafka periodically
  - the cache driver will receive the event and persist the usage by cluster

- Billing driver(management/cron)
  - it will execute one time per month to send an invoice
  - it will use the usage in the cache to calculate the amount to billing
  - Fetch all projects
  - Fetch all ports of a project
  - Fetch usage by port
  - Integrate with payment gateway(send invoice)
