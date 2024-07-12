locals {
  console_instance_name  = "fabric-queue-console"
  console_configmap_name = "${local.console_instance_name}-config"
}

resource "kubernetes_config_map_v1" "fabric_queue_console_config" {
  metadata {
    name      = local.console_configmap_name
    namespace = var.namespace
  }

  data = {
    "config.yml" = "${templatefile(
      "${path.module}/console_config.yml",
      {
        instance_name                 = var.instance_name,
        kafka_external_port           = local.kafka_external_port,
        schema_registry_external_port = local.schema_registry_external_port,
        admin_api_external_port       = local.admin_api_external_port
      }
    )}"
  }
}

resource "kubernetes_deployment_v1" "fabric_queue_console" {
  wait_for_rollout = false
  depends_on       = [kubernetes_config_map_v1.fabric_queue_console_config]

  metadata {
    name      = local.console_instance_name
    namespace = var.namespace
  }

  spec {
    replicas = 1

    selector {
      match_labels = {
        "demeter.run/instance" : local.console_instance_name
      }
    }

    template {
      metadata {
        labels = {
          "demeter.run/instance" : local.console_instance_name
        }
      }

      spec {
        container {
          name  = "main"
          image = var.console_image

          env {
            name  = "CONFIG_FILEPATH"
            value = "/etc/config/config.yml"
          }

          volume_mount {
            name       = "config"
            mount_path = "/etc/config"
          }

          resources {
            limits = {
              cpu    = var.console_resources.limits.cpu
              memory = var.console_resources.limits.memory
            }
            requests = {
              cpu    = var.console_resources.requests.cpu
              memory = var.console_resources.requests.memory
            }
          }

        }

        volume {
          name = "config"
          config_map {
            name = local.console_configmap_name
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
