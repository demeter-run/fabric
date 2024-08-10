locals {
  configmap_name      = "fabric-rpc-config"
  crds_configmap_name = "fabric-rpc-crds"
  port                = 5050
}

variable "namespace" {
  type = string
}

variable "image" {
  type = string
}

variable "broker_urls" {
  type        = string
  description = "Comma separated values of the queue broker urls."
}

variable "consumer_name" {
  type = string
}

variable "kafka_username" {
  type = string
}

variable "kafka_password" {
  type = string
}

variable "kafka_topic" {
  type    = string
  default = "events"
}

variable "secret" {
  type = string
}

variable "replicas" {
  type    = number
  default = 1
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
    storage = object({
      size  = string
      class = string
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
    storage = {
      size  = "10Gi"
      class = "fast"
    }
  }
}
