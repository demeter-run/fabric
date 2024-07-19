locals {
  create_topic_job_name = "fabric-queue-create-topic"
}

resource "kubernetes_job_v1" "fabric_queue_create_topic" {
  depends_on = [helm_release.redpanda]

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
          image = "docker.redpanda.com/redpandadata/redpanda:v23.3.18"
          command = [
            "rpk",
            "-X", "sasl.mechanism=SCRAM-SHA-256",
            "-X", "user=${var.admin_username}",
            "-X", "pass=${var.admin_password}",
            "topic", "create", "events",
            "-r", "${var.replicas}",
            "-c", "cleanup.policy=compact",
            "-c", "retention.ms=-1",
          ]
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

          resources {
            limits = {
              cpu    = "500m"
              memory = "512Mi"
            }
            requests = {
              cpu    = "500m"
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
