variable "namespace" {
  type = string
}

variable "instance_name" {
  type = string
}

variable "admin_username" {
  type = string
}

variable "admin_password" {
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
