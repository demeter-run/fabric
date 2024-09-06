terraform {
  backend "s3" {
    bucket = "demeter-tf"
    key    = "github/demeter-fabric.tfstate"
    region = "us-west-2"
  }
  required_providers {
    kubernetes = {
      source = "hashicorp/kubernetes"
    }
  }
}

provider "kubernetes" {
  config_path    = "~/.kube/config"
  config_context = "arn:aws:eks:us-west-2:295114534192:cluster/m2-prod-7xjh33"
}

provider "helm" {
  kubernetes {
    config_path    = "~/.kube/config"
    config_context = "arn:aws:eks:us-west-2:295114534192:cluster/m2-prod-7xjh33"
  }
}

variable "rpc_image" {}
variable "kafka_admin_password" {}
variable "kafka_rpc_password" {}
variable "kafka_daemon_password" {}
variable "secret" {}
variable "auth0_client_id" {}
variable "auth0_client_secret" {}
variable "auth0_audience" {}
variable "stripe_api_key" {}
variable "email_ses_access_key_id" {}
variable "email_ses_secret_access_key" {}

locals {
  namespace                   = "fabric-stg"
  queue_instance_name         = "fabric-queue"
  replicas                    = 1
  external_domain             = "stg-fabric-queue.demeter.run"
  broker_urls                 = "redpanda.${local.external_domain}:31092"
  secret                      = var.secret
  kafka_admin_username        = "admin"
  kafka_admin_password        = var.kafka_admin_password
  kafka_rpc_username          = "rpc"
  kafka_rpc_password          = var.kafka_rpc_password
  kafka_daemon_username       = "daemon"
  kafka_daemon_password       = var.kafka_daemon_password
  kafka_daemon_consumer       = "daemon"
  kafka_topic                 = "stg"
  auth0_client_id             = var.auth0_client_id
  auth0_client_secret         = var.auth0_client_secret
  auth0_audience              = var.auth0_audience
  stripe_api_key              = var.stripe_api_key
  email_invite_ttl_min        = 15
  email_ses_region            = "us-west-2"
  email_ses_access_key_id     = var.email_ses_access_key_id
  email_ses_secret_access_key = var.email_ses_secret_access_key
  email_ses_verified_email    = "no-reply@demeter.run"
}

resource "kubernetes_namespace_v1" "fabric_namespace" {
  metadata {
    name = local.namespace
  }
}

module "fabric_queue" {
  source     = "../../../fabric/bootstrap/queue/"
  depends_on = [kubernetes_namespace_v1.fabric_namespace]

  namespace       = local.namespace
  instance_name   = local.queue_instance_name
  replicas        = local.replicas
  external_domain = local.external_domain
  admin_username  = local.kafka_admin_username
  admin_password  = local.kafka_admin_password
  rpc_username    = local.kafka_rpc_username
  rpc_password    = local.kafka_rpc_password

  daemon_users = [
    {
      name          = local.kafka_daemon_username
      password      = local.kafka_daemon_password
      consumer_name = local.kafka_daemon_consumer
    },
  ]
}

module "fabric_rpc" {
  source = "../../../fabric/bootstrap/rpc"

  namespace                   = local.namespace
  image                       = var.rpc_image
  broker_urls                 = local.broker_urls
  consumer_name               = "rpc"
  kafka_username              = local.kafka_admin_username
  kafka_password              = local.kafka_admin_password
  kafka_topic                 = local.kafka_topic
  secret                      = local.secret
  auth0_client_id             = local.auth0_client_id
  auth0_client_secret         = local.auth0_client_secret
  auth0_audience              = local.auth0_audience
  stripe_api_key              = local.stripe_api_key
  email_invite_ttl_min        = local.email_invite_ttl_min
  email_ses_region            = local.email_ses_region
  email_ses_access_key_id     = local.email_ses_access_key_id
  email_ses_secret_access_key = local.email_ses_secret_access_key
  email_ses_verified_email    = local.email_ses_verified_email
}

module "fabric_services" {
  source     = "../../../fabric/bootstrap/services/"
  depends_on = [module.fabric_queue, module.fabric_rpc]

  namespace          = local.namespace
  ingress_class_name = "nginx"
  dns_zone           = "demeter.run"
}
