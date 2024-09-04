variable "namespace" {
  type = string
}

variable "instance_name" {
  type = string
}

variable "admin_username" {
  type    = string
  default = "admin"
}

variable "admin_password" {
  type = string
}

variable "rpc_username" {
  type    = string
  default = "rpc"
}

variable "rpc_password" {
  type = string
}

variable "daemon_users" {
  type = list(object({
    name          = string
    password      = string
    consumer_name = string
  }))
}

variable "external_domain" {
  type = string
}

variable "image_repository" {
  type    = string
  default = "docker.redpanda.com/redpandadata/redpanda"
}

variable "image_tag" {
  type    = string
  default = "v23.3.18"
}

variable "resources" {
  type = object({
    cpu     = number
    memory  = string
    storage = string
  })
  default = {
    cpu     = 1
    memory  = "2.5Gi"
    storage = "20Gi"
  }
}

variable "replicas" {
  type    = number
  default = 3
}

variable "replication" {
  type    = number
  default = null
}

variable "tolerations" {
  type = list(object({
    effect   = string
    key      = string
    operator = string
    value    = optional(string)
  }))
  default = [
    {
      effect   = "NoSchedule"
      key      = "demeter.run/compute-profile"
      operator = "Exists"
    },
    {
      effect   = "NoSchedule"
      key      = "demeter.run/compute-arch"
      operator = "Exists"
    },
    {
      effect   = "NoSchedule"
      key      = "demeter.run/availability-sla"
      operator = "Equal"
      value    = "consistent"
    }

  ]
}
