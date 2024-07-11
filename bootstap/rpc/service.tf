resource "kubernetes_service_v1" "fabric_rpc_service" {
  metadata {
    namespace = var.namespace
    name      = "fabric-rpc"
  }

  spec {
    type = "ClusterIP"

    port {
      name        = "grpc"
      port        = local.port
      protocol    = "TCP"
      target_port = local.port
    }

    selector = {
      role = local.role
    }
  }
}
