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
variable "daemon_image" {}
variable "kafka_rpc_password" {}
variable "kafka_daemon_password" {}
variable "secret" {}
variable "auth0_client_id" {}
variable "auth0_client_secret" {}
variable "auth0_audience" {}
variable "stripe_api_key" {}
variable "slack_webhook_url" {}
variable "email_ses_access_key_id" {}
variable "email_ses_secret_access_key" {}


locals {
  rpc_namespace               = "demeter-global"
  daemon_namespace            = "demeter-fabric"
  replicas                    = 1
  broker_urls                 = "csihobu921nrmk52flsg.any.us-east-1.mpx.prd.cloud.redpanda.com:9092"
  secret                      = var.secret
  kafka_rpc_username          = "rpc"
  kafka_rpc_password          = var.kafka_rpc_password
  kafka_daemon_username       = "daemon-us-west-2-m2"
  kafka_daemon_password       = var.kafka_daemon_password
  kafka_topic                 = "events"
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

module "fabric_rpc" {
  source = "../../bootstrap/rpc"

  namespace                   = local.rpc_namespace
  image                       = var.rpc_image
  broker_urls                 = local.broker_urls
  consumer_name               = "rpc-ahid01"
  kafka_username              = local.kafka_rpc_username
  kafka_password              = local.kafka_rpc_password
  kafka_topic                 = local.kafka_topic
  secret                      = local.secret
  auth0_client_id             = local.auth0_client_id
  auth0_client_secret         = local.auth0_client_secret
  auth0_audience              = local.auth0_audience
  stripe_api_key              = local.stripe_api_key
  slack_webhook_url           = var.slack_webhook_url
  email_invite_ttl_min        = local.email_invite_ttl_min
  email_ses_region            = local.email_ses_region
  email_ses_access_key_id     = local.email_ses_access_key_id
  email_ses_secret_access_key = local.email_ses_secret_access_key
  email_ses_verified_email    = local.email_ses_verified_email
  url_prefix                  = "rpc"
}

module "fabric_daemon" {
  source = "../../bootstrap/daemon"

  namespace             = local.daemon_namespace
  image                 = var.daemon_image
  cluster_id            = "txpipe-us-east-2-m2"
  broker_urls           = local.broker_urls
  consumer_monitor_name = "daemon-us-west-2-m2-monitor-hi1"
  consumer_cache_name   = "daemon-us-west-2-m2-cache-hi1"
  kafka_username        = local.kafka_daemon_username
  kafka_password        = local.kafka_daemon_password
  kafka_topic           = local.kafka_topic
  mode                  = "usage"
}
