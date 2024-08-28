locals {
  configmap_name = "fabric-daemon-config"
  port           = 5000
}

variable "namespace" {
  type = string
}

variable "image" {
  type = string
}

variable "cluster_id" {
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

variable "replicas" {
  type    = number
  default = 1
}

variable "prometheus_url" {
  type        = string
  description = "URL where to query prometheus to report usage metrics."
  default     = "http://prometheus-operated.demeter-system.svc.cluster.local:9090/api/v1"
}

variable "prometheus_delay_sec" {
  type        = number
  description = "Delay between usage report loops."
  default     = 60
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
