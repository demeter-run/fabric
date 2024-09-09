locals {
  cert_secret_name = "rpc-tls"
}

resource "kubernetes_manifest" "certificate_cluster_wildcard_tls" {
  manifest = {
    "apiVersion" = "cert-manager.io/v1"
    "kind"       = "Certificate"
    "metadata" = {
      "name"      = local.cert_secret_name
      "namespace" = var.namespace
    }
    "spec" = {
      "dnsNames" = [
        "${var.url_prefix}.${var.dns_zone}"
      ]

      "issuerRef" = {
        "kind" = "ClusterIssuer"
        "name" = "letsencrypt-dns01"
      }
      "secretName" = local.cert_secret_name
    }
  }
}

