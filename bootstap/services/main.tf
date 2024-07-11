variable "namespace" {
  type = string
}

variable "ingress_class_name" {
  type    = string
  default = "nginx"
}

variable "dns_zone" {
  type    = string
  default = "demeter.run"
}

resource "kubernetes_service_v1" "fabric-queue-load-balancer" {
  metadata {
    namespace = var.namespace
    name      = "fabric-queue-load-balancer"
    annotations = {
      "beta.kubernetes.io/aws-load-balancer-nlb-target-type" = "instance"
      "service.beta.kubernetes.io/aws-load-balancer-scheme"  = "internet-facing"
      "service.beta.kubernetes.io/aws-load-balancer-type"    = "external"
    }
  }

  spec {
    type                = "LoadBalancer"
    load_balancer_class = "service.k8s.aws/nlb"

    port {
      protocol    = "TCP"
      port        = 9092
      target_port = 19092
    }

    selector = {
      "role" = "fabric-queue-leader"
    }
  }
}

resource "kubernetes_ingress_v1" "fabric-rpc-ingress" {
  wait_for_load_balancer = true
  metadata {
    name      = "fabric-rpc-ingress"
    namespace = var.namespace
    annotations = {
      "cert-manager.io/cluster-issuer" = "letsencrypt"
    }
  }

  spec {
    ingress_class_name = var.ingress_class_name

    rule {
      host = "rpc.${var.dns_zone}"
      http {
        path {
          path = "/"

          backend {
            service {
              name = "fabric-rpc"
              port {
                number = 5050
              }
            }
          }
        }
      }
    }
    tls {
      hosts = ["rpc.${var.dns_zone}"]
    }
  }
}
