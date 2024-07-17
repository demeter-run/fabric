resource "helm_release" "redpanda" {
  name             = "redpanda"
  repository       = "https://charts.redpanda.com"
  chart            = "redpanda/redpanda"
  create_namespace = false
  namespace        = var.namespace
  values           = [file("${path.module}/values.yaml")]

  set {
    name  = "nameOverride"
    value = var.instance_name
  }

  set {
    name  = "image.tag"
    value = var.image_tag
  }

  set {
    name  = "resources.cpu.cores"
    value = var.resources.cpu
  }

  set {
    name  = "memory.container.max"
    value = var.resources.memory
  }

  set {
    name  = "storage.persistentVolume.size"
    value = var.resources.storage
  }

  set {
    name  = "statefulset.replicas"
    value = var.replicas
  }
}
