resource "kubernetes_manifest" "rpc_monitor" {
  manifest = {
    apiVersion = "monitoring.coreos.com/v1"
    kind       = "PodMonitor"
    metadata = {
      labels = {
        "app.kubernetes.io/component" = "o11y"
        "app.kubernetes.io/part-of"   = "demeter"
      }
      name      = "fabric-rpc"
      namespace = var.namespace
    }
    spec = {
      selector = {
        matchLabels = {
          role = "fabric-rpc"
        }
      }
      podMetricsEndpoints = [
        {
          port = "metrics",
          path = "/metrics"
        }
      ]
    }
  }
}
