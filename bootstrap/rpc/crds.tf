resource "kubernetes_config_map_v1" "fabric_rpc_crds" {
  metadata {
    name      = local.crds_configmap_name
    namespace = var.namespace
  }

  data = {
    "utxorpcport.json" = "${file("${path.module}/utxorpcport.json")}"
  }
}
