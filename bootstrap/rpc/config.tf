resource "kubernetes_config_map_v1" "fabric_rpc_config" {
  metadata {
    name      = local.configmap_name
    namespace = var.namespace
  }

  data = {
    "rpc.toml" = "${templatefile(
      "${path.module}/rpc.toml.tftpl",
      {
        port = local.port,
        // If we change the consumer, we must rebuild the cache.
        db_path                     = "/var/cache/${var.consumer_name}.db",
        broker_urls                 = var.broker_urls
        consumer_name               = var.consumer_name
        kafka_username              = var.kafka_username
        kafka_password              = var.kafka_password
        topic                       = var.kafka_topic
        secret                      = var.secret
        auth0_client_id             = var.auth0_client_id
        auth0_client_secret         = var.auth0_client_secret
        auth0_audience              = var.auth0_audience
        stripe_api_key              = var.stripe_api_key
        slack_webhook_url           = var.slack_webhook_url
        email_invite_ttl_min        = var.email_invite_ttl_min
        email_ses_access_key_id     = var.email_ses_access_key_id
        email_ses_secret_access_key = var.email_ses_secret_access_key
        email_ses_region            = var.email_ses_region
        email_ses_verified_email    = var.email_ses_verified_email
        prometheus_addr             = var.prometheus_addr
      }
    )}"
  }
}
