resource "kubernetes_config_map_v1" "fabric_rpc_config" {
  metadata {
    name      = local.configmap_name
    namespace = var.namespace
  }

  data = {
    "rpc.toml" = "${templatefile(
      "${path.module}/rpc.toml",
      {
        port        = local.port,
        db_path     = "cache.db",
        broker_urls = var.broker_urls
      }
    )}"
  }
}


