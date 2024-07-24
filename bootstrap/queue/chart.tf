resource "helm_release" "redpanda" {
  name             = "redpanda"
  repository       = "https://charts.redpanda.com"
  chart            = "redpanda"
  version          = "5.7.35"
  create_namespace = false
  namespace        = var.namespace
  values = [templatefile(
    "${path.module}/values.yml.tftpl",
    {
      admin_username = var.admin_username
      admin_password = var.admin_password
      rpc_username   = var.rpc_username
      rpc_password   = var.rpc_password
      daemon_users   = var.daemon_users
    }
  )]

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

  set {
    name  = "external.domain"
    value = var.external_domain
  }
}
