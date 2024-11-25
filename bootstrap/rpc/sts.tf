locals {
  role = "fabric-rpc"
}

resource "kubernetes_stateful_set_v1" "rpc" {
  wait_for_rollout = false

  metadata {
    labels = {
      role = local.role
    }
    name      = "fabric-rpc"
    namespace = var.namespace
  }

  spec {
    replicas     = var.replicas
    service_name = "fabric-rpc"

    volume_claim_template {
      metadata {
        name = "cache"
      }
      spec {
        access_modes = ["ReadWriteOnce"]
        resources {
          requests = {
            storage = var.resources.storage.size
          }
        }
        storage_class_name = var.resources.storage.class
      }
    }

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
          name  = "rpc"
          image = var.image

          env {
            name  = "RPC_CONFIG"
            value = "/fabric/config/rpc.toml"
          }

          port {
            name           = "api"
            container_port = local.port
          }

          port {
            name           = "metrics"
            container_port = local.prometheus_port
          }

          volume_mount {
            name       = "cache"
            mount_path = "/var/cache"
          }

          volume_mount {
            name       = "config"
            mount_path = "/fabric/config"
          }

          volume_mount {
            name       = "crds"
            mount_path = "/fabric/crds"
          }

          volume_mount {
            mount_path = "/certs"
            name       = "certs"
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

        volume {
          name = "crds"
          config_map {
            name = local.crds_configmap_name
          }
        }

        volume {
          name = "certs"
          secret {
            secret_name = local.cert_secret_name
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
