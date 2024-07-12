locals {
  role = "fabric-daemon"
}

resource "kubernetes_deployment_v1" "daemon" {
  wait_for_rollout = false

  metadata {
    labels = {
      role = local.role
    }
    name      = "fabric-daemon"
    namespace = var.namespace
  }

  spec {
    replicas = 1

    selector {
      match_labels = {
        role = local.role
      }
    }

    template {
      metadata {
        labels = {
          role = local.role
        }
      }

      spec {
        container {
          name  = "daemon"
          image = var.image

          env {
            name  = "DAEMON_CONFIG"
            value = "/fabric/daemon.toml"
          }

          port {
            container_port = local.port
          }

          volume_mount {
            name       = "config"
            mount_path = "/fabric"
          }

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

        volume {
          name = "config"
          config_map {
            name = local.configmap_name
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
