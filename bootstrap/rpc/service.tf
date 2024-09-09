resource "kubernetes_service_v1" "service" {
  metadata {
    name      = "rpc"
    namespace = var.namespace
    annotations = {
      "service.beta.kubernetes.io/aws-load-balancer-nlb-target-type" : "instance"
      "service.beta.kubernetes.io/aws-load-balancer-scheme" : "internet-facing"
      "service.beta.kubernetes.io/aws-load-balancer-type" : "external"
    }
  }

  spec {
    load_balancer_class = "service.k8s.aws/nlb"
    selector = {
      role = local.role
    }

    port {
      name        = "api"
      port        = 443
      target_port = local.port
      protocol    = "TCP"
    }

    type = "LoadBalancer"
  }
}
