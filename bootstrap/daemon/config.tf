resource "kubernetes_config_map_v1" "fabric_daemon_config" {
  metadata {
    name      = local.configmap_name
    namespace = var.namespace
  }

  data = {
    "daemon.toml" = "${templatefile(
      "${path.module}/daemon.toml.tftpl",
      {
        broker_urls    = var.broker_urls
        consumer_name  = var.consumer_name
        kafka_username = var.kafka_username
        kafka_password = var.kafka_password
        topic          = var.kafka_topic
      }
    )}"
  }
}


