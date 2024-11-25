locals {
  configmap_name      = "fabric-rpc-config"
  crds_configmap_name = "fabric-rpc-crds"
  port                = 5050
  prometheus_port     = 9946
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

variable "auth0_client_id" {
  type = string
}

variable "auth0_client_secret" {
  type = string
}

variable "auth0_audience" {
  type = string
}

variable "stripe_api_key" {
  type = string
}

variable "slack_webhook_url" {
  type    = string
  default = null
}

variable "kafka_topic" {
  type    = string
  default = "events"
}

variable "secret" {
  type = string
}

variable "email_invite_ttl_min" {
  type    = number
  default = 15
}

variable "email_ses_access_key_id" {
  type = string
}

variable "email_ses_secret_access_key" {
  type = string
}

variable "email_ses_region" {
  type    = string
  default = "us-west-2"
}

variable "email_ses_verified_email" {
  type    = string
  default = "no-reply@demeter.run"
}

variable "url_prefix" {
  type    = string
  default = "rpc"
}

variable "dns_zone" {
  type    = string
  default = "demeter.run"
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
