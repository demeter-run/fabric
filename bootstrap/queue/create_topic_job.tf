locals {
  create_topic_job_name = "fabric-queue-create-topic"
  brokers = join(",", [
    for i in range(1, var.replicas) : "redpanda-${i}.${var.external_domain}:31092"
  ])
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
            "-X",
            "brokers=${local.brokers}",
            "topic",
            "create",
            "events",
            "-r", "3",
            "-c", "cleanup.policy=compact",
            "-c", "retention.ms=-1",
          ]
          image_pull_policy = "Always"

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
