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

variable "users" {
  type = list(object({
    name     = string
    password = string
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

variable "console_image" {
  type    = string
  default = "docker.redpanda.com/redpandadata/console:v2.3.1"
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
