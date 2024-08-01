resource "kubernetes_config_map_v1" "fabric_rpc_config" {
  metadata {
    name      = local.configmap_name
    namespace = var.namespace
  }

  data = {
    "rpc.toml" = "${templatefile(
      "${path.module}/rpc.toml.tftpl",
      {
        port           = local.port,
        db_path        = "cache.db",
        broker_urls    = var.broker_urls
        consumer_name  = var.consumer_name
        kafka_username = var.kafka_username
        kafka_password = var.kafka_password
        topic          = var.kafka_topic
        secret         = var.secret
      }
    )}"
  }
}


