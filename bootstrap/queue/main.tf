locals {
  kafka_internal_port           = 9092
  kafka_external_port           = 19092
  pandaproxy_internal_port      = 8082
  pandaproxy_external_port      = 18082
  schema_registry_internal_port = 8081
  schema_registry_external_port = 18081
  rpc_port                      = 33145
  admin_api_external_port       = 19644
}

variable "namespace" {
  type = string
}

variable "instance_name" {
  type = string
}

variable "external_dns" {
  type = string
}

variable "external_port" {
  type    = string
  default = 9092
}

variable "topology_zone" {
  type    = string
  default = null
}

variable "image" {
  type = string
}

variable "console_image" {
  type    = string
  default = "docker.redpanda.com/redpandadata/console:v2.3.1"
}

variable "resources" {
  type = object({
    limits = object({
      cpu    = optional(string)
      memory = string
    })
    requests = object({
      cpu    = string
      memory = string
    })
  })
  default = {
    requests = {
      cpu    = "100m"
      memory = "1G"
    }
    limits = {
      cpu    = "4"
      memory = "1G"
    }
  }
}

variable "console_resources" {
  type = object({
    limits = object({
      cpu    = optional(string)
      memory = string
    })
    requests = object({
      cpu    = string
      memory = string
    })
  })
  default = {
    requests = {
      cpu    = "100m"
      memory = "500Mi"
    }
    limits = {
      cpu    = "500m"
      memory = "500Mi"
    }
  }
}

variable "tolerations" {
  type = list(object({
    effect   = string
    key      = string
    operator = string
    value    = string
  }))
  default = [
    {
      effect   = "NoSchedule"
      key      = "demeter.run/compute-profile"
      operator = "Equal"
      value    = "general-purpose"
    },
    {
      effect   = "NoSchedule"
      key      = "demeter.run/compute-arch"
      operator = "Equal"
      value    = "x86"
    },
    {
      effect   = "NoSchedule"
      key      = "demeter.run/availability-sla"
      operator = "Equal"
      value    = "consistent"
    }

  ]
}
