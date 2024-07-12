resource "kubernetes_service_v1" "fabric_queue_service" {
  metadata {
    namespace = var.namespace
    name      = var.instance_name
  }

  spec {
    type = "ClusterIP"

    port {
      name        = "schema-registry-internal"
      protocol    = "TCP"
      port        = local.schema_registry_internal_port
      target_port = local.schema_registry_internal_port
    }

    port {
      name        = "schema-registry-external"
      protocol    = "TCP"
      port        = local.schema_registry_external_port
      target_port = local.schema_registry_external_port
    }

    port {
      name        = "pandaproxy-internal"
      protocol    = "TCP"
      port        = local.pandaproxy_internal_port
      target_port = local.pandaproxy_internal_port
    }

    port {
      name        = "pandaproxy-external"
      protocol    = "TCP"
      port        = local.pandaproxy_external_port
      target_port = local.pandaproxy_external_port
    }

    port {
      name        = "kafka-internal"
      protocol    = "TCP"
      port        = local.kafka_internal_port
      target_port = local.kafka_internal_port
    }

    port {
      name        = "kafka-external"
      protocol    = "TCP"
      port        = local.kafka_external_port
      target_port = local.kafka_external_port
    }

    port {
      name        = "rpc"
      protocol    = "TCP"
      port        = local.rpc_port
      target_port = local.rpc_port
    }

    port {
      name        = "admin-api-external"
      port        = local.admin_api_external_port
      protocol    = "TCP"
      target_port = 9644
    }

    selector = {
      "demeter.run/instance" = var.instance_name
    }
  }
}
