resource "kubernetes_config_map_v1" "fabric_rpc_crds" {
  metadata {
    name      = local.crds_configmap_name
    namespace = var.namespace
  }

  data = {
    "blockfrostport.hbs"   = "${file("${path.module}/crds/blockfrostport.hbs")}"
    "blockfrostport.json"  = "${file("${path.module}/crds/blockfrostport.json")}"
    "cardanonodeport.hbs"  = "${file("${path.module}/crds/cardanonodeport.hbs")}"
    "cardanonodeport.json" = "${file("${path.module}/crds/cardanonodeport.json")}"
    "dbsyncport.hbs"       = "${file("${path.module}/crds/dbsyncport.hbs")}"
    "dbsyncport.json"      = "${file("${path.module}/crds/dbsyncport.json")}"
    "frontends.json"       = "${file("${path.module}/crds/frontends.json")}"
    "kupoport.hbs"         = "${file("${path.module}/crds/kupoport.hbs")}"
    "kupoport.json"        = "${file("${path.module}/crds/kupoport.json")}"
    "marloweport.hbs"      = "${file("${path.module}/crds/marloweport.hbs")}"
    "marloweport.json"     = "${file("${path.module}/crds/marloweport.json")}"
    "mumakport.hbs"        = "${file("${path.module}/crds/mumakport.hbs")}"
    "mumakport.json"       = "${file("${path.module}/crds/mumakport.json")}"
    "ogmiosport.hbs"       = "${file("${path.module}/crds/ogmiosport.hbs")}"
    "ogmiosport.json"      = "${file("${path.module}/crds/ogmiosport.json")}"
    "scrollsport.hbs"      = "${file("${path.module}/crds/scrollsport.hbs")}"
    "scrollsport.json"     = "${file("${path.module}/crds/scrollsport.json")}"
    "submitapiport.hbs"    = "${file("${path.module}/crds/submitapiport.hbs")}"
    "submitapiport.json"   = "${file("${path.module}/crds/submitapiport.json")}"
    "utxorpcport.hbs"      = "${file("${path.module}/crds/utxorpcport.hbs")}"
    "utxorpcport.json"     = "${file("${path.module}/crds/utxorpcport.json")}"
  }
}
