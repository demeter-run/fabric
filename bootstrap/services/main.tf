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

resource "kubernetes_ingress_v1" "fabric-rpc-ingress" {
  wait_for_load_balancer = true
  metadata {
    name      = "fabric-rpc-ingress"
    namespace = var.namespace
    annotations = {
      "cert-manager.io/cluster-issuer" = "letsencrypt"
      "nginx.ingress.kubernetes.io/backend-protocol" : "GRPC"
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
      hosts       = ["rpc.${var.dns_zone}"]
      secret_name = "rpc-tls"
    }
  }
}
