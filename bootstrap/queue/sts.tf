locals {
  args = [
    "redpanda",
    "start",
    "--kafka-addr",
    "internal://${var.instance_name}-0:${local.kafka_internal_port},external://0.0.0.0:${local.kafka_external_port}",
    "--advertise-kafka-addr",
    "internal://${var.instance_name}:${local.kafka_internal_port},external://${var.external_dns}:${var.external_port}",
    "--pandaproxy-addr",
    "internal://${var.instance_name}-0:${local.pandaproxy_internal_port},external://0.0.0.0:${local.pandaproxy_external_port}",
    "--advertise-pandaproxy-addr",
    "internal://${var.instance_name}:${local.pandaproxy_internal_port},external://${var.instance_name}:${local.pandaproxy_external_port}",
    "--schema-registry-addr",
    "internal://${var.instance_name}-0:${local.schema_registry_internal_port},external://0.0.0.0:${local.schema_registry_external_port}",
    "--rpc-addr",
    "${var.instance_name}-0:${local.rpc_port}",
    "--advertise-rpc-addr", "${var.instance_name}:${local.rpc_port}",
    "--smp", "1",
    "--reserve-memory", "0M",
  ]

  create_topic_job_name = "${var.instance_name}-create-topic"
}

resource "kubernetes_stateful_set_v1" "queue_main" {
  metadata {
    name      = var.instance_name
    namespace = var.namespace
  }
  spec {
    replicas     = 1
    service_name = "fabric-queue"
    selector {
      match_labels = {
        "demeter.run/instance" = var.instance_name
        "role"                 = "fabric-queue-leader"
      }
    }

    template {
      metadata {
        labels = {
          "demeter.run/instance" = var.instance_name
          "role"                 = "fabric-queue-leader"
        }
      }
      spec {
        dynamic "affinity" {
          for_each = var.topology_zone != null ? toset([1]) : toset([])

          content {
            node_affinity {
              required_during_scheduling_ignored_during_execution {
                node_selector_term {
                  match_expressions {
                    key      = "topology.kubernetes.io/zone"
                    operator = "In"
                    values   = [var.topology_zone]
                  }
                }
              }
            }
          }
        }

        security_context {
          fs_group = 1000
        }

        container {
          name              = "main"
          image             = var.image
          args              = concat(local.args, ["--node-id", "0"])
          image_pull_policy = "Always"

          resources {
            limits = {
              cpu    = var.resources.limits.cpu
              memory = var.resources.limits.memory
            }
            requests = {
              cpu    = var.resources.requests.cpu
              memory = var.resources.requests.memory
            }
          }
        }

        dynamic "toleration" {
          for_each = var.tolerations

          content {
            effect   = toleration.value.effect
            key      = toleration.value.key
            operator = toleration.value.operator
            value    = toleration.value.value
          }
        }
      }
    }
  }
}


resource "kubernetes_job_v1" "fabric_queue_create_topic" {
  depends_on = [kubernetes_stateful_set_v1.queue_main]

  metadata {
    name      = local.create_topic_job_name
    namespace = var.namespace
  }
  spec {
    template {
      metadata {
        labels = {
          "demeter.run/instance" = local.create_topic_job_name
        }
      }
      spec {
        security_context {
          fs_group = 1000
        }

        container {
          name  = "main"
          image = var.image
          command = [
            "rpk",
            "-X",
            "brokers=${var.instance_name}:${local.kafka_external_port}",
            "topic",
            "create",
            "events"
          ]
          image_pull_policy = "Always"

          resources {
            limits = {
              cpu    = var.resources.limits.cpu
              memory = var.resources.limits.memory
            }
            requests = {
              cpu    = var.resources.requests.cpu
              memory = var.resources.requests.memory
            }
          }
        }

        dynamic "toleration" {
          for_each = var.tolerations

          content {
            effect   = toleration.value.effect
            key      = toleration.value.key
            operator = toleration.value.operator
            value    = toleration.value.value
          }
        }
      }
    }
  }

}
