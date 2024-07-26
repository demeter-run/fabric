locals {
  setup_job_name       = "fabric-queue-setup"
  setup_configmap_name = "fabric-queue-setup-config"
  replication          = coalesce(var.replication, var.replicas)
  events_topic         = "events"
}

resource "kubernetes_config_map_v1" "fabric_queue_setup_config" {
  metadata {
    name      = local.setup_configmap_name
    namespace = var.namespace
  }

  data = {
    "setup.sh" = "${templatefile(
      "${path.module}/setup.sh.tftpl",
      {
        admin_username = var.admin_username
        admin_password = var.admin_password
        rpc_username   = var.rpc_username
        rpc_password   = var.rpc_password
        replication    = local.replication
        events_topic   = local.events_topic
        daemon_users   = var.daemon_users
      }
    )}"
  }
}


resource "kubernetes_job_v1" "fabric_queue_setup" {
  depends_on = [helm_release.redpanda, kubernetes_config_map_v1.fabric_queue_setup_config]

  metadata {
    name      = local.setup_job_name
    namespace = var.namespace
  }
  spec {
    template {
      metadata {
        labels = {
          "demeter.run/instance" = local.setup_job_name
        }
      }
      spec {
        security_context {
          fs_group = 1000
        }

        container {
          name              = "main"
          image             = "docker.redpanda.com/redpandadata/redpanda:v23.3.18"
          command           = ["/bin/sh", "/var/setup/setup.sh"]
          image_pull_policy = "Always"

          volume_mount {
            name       = "redpanda-default-cert"
            mount_path = "/etc/tls/certs/default"
          }

          volume_mount {
            name       = "redpanda-external-cert"
            mount_path = "/etc/tls/certs/external"
          }

          volume_mount {
            name       = "config"
            mount_path = "/etc/redpanda"
          }

          volume_mount {
            name       = "setup"
            mount_path = "/var/setup"
          }

          resources {
            limits = {
              cpu    = "200m"
              memory = "512Mi"
            }
            requests = {
              cpu    = "200m"
              memory = "512Mi"
            }
          }
        }

        volume {
          name = "redpanda-default-cert"
          secret {
            secret_name = "redpanda-default-cert"
          }
        }

        volume {
          name = "redpanda-external-cert"
          secret {
            secret_name = "redpanda-external-cert"
          }
        }

        volume {
          name = "config"
          config_map {
            name = "redpanda"
          }
        }

        volume {
          name = "setup"
          config_map {
            name = local.setup_configmap_name
          }
        }

        toleration {
          effect   = "NoSchedule"
          key      = "demeter.run/compute-profile"
          operator = "Equal"
          value    = "general-purpose"
        }

        toleration {
          effect   = "NoSchedule"
          key      = "demeter.run/compute-arch"
          operator = "Equal"
          value    = "x86"
        }
      }
    }
  }

}
