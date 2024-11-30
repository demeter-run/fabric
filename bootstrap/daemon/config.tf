resource "kubernetes_config_map_v1" "fabric_daemon_config" {
  metadata {
    name      = local.configmap_name
    namespace = var.namespace
  }

  data = {
    "daemon.toml" = "${templatefile(
      "${path.module}/daemon.toml.tftpl",
      {
        db_path               = "/var/cache/${var.consumer_cache_name}.db",
        broker_urls           = var.broker_urls
        consumer_cache_name   = var.consumer_cache_name
        consumer_monitor_name = var.consumer_monitor_name
        kafka_username        = var.kafka_username
        kafka_password        = var.kafka_password
        topic                 = var.kafka_topic
        cluster_id            = var.cluster_id
        prometheus_url        = var.prometheus_url
        prometheus_delay_sec  = var.prometheus_delay_sec
        prometheus_query_step = var.prometheus_query_step
        mode                  = var.mode
        metrics_port          = local.metrics_port
      }
    )}"
  }
}


