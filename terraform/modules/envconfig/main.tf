module "legacy" {
  source = "github.com/Appreciate-Stuff/appr-tfmod-infra-legacy?ref=v0.1.0"
}

locals {
  namespace       = "app"
  product         = "platform"
  component       = "delegator"
  aws_region      = var.aws_region
  env             = var.env
  image_name      = local.component
  image_tag       = var.image_tag
  container_port  = 80

  domain_name     = "appreciate.it"
  subdomain_names = {
    stag        = "staging.${local.domain_name}"
    prod        = local.domain_name
    thunderdome = "thunderdome.${local.domain_name}"
  }
}

module "label" {
  source = "github.com/Appreciate-Stuff/appr-tfmod-resource-label?ref=v1.1.0"

  namespace   = local.namespace
  product     = local.product
  component   = local.component
  region      = local.aws_region
  environment = local.env
  
  tags = {
    "Created-By"  = "terraform"
    "Environment" = local.env
    "Region"      = local.aws_region
    "Product"     = local.product
    "Component"   = local.component
  }
}

data "aws_acm_certificate" "ssl_cert" {
  domain      = local.domain_name
  types       = ["AMAZON_ISSUED"]
  most_recent = true
}

#-------------------------------------------------------------------
# Security Group 
#-------------------------------------------------------------------

resource "aws_security_group" "main" {
  name        = "${module.label.id}-alb-sg"
  description = "Security group for ALB"
  vpc_id      = module.legacy.vpcs[local.env].id

  tags = merge(module.label.tags, {
    "Name" = "${module.label.id}-alb-sg"
  })
}

resource "aws_security_group_rule" "egress" {
  type              = "egress"
  from_port         = 0
  to_port           = 0
  protocol          = "-1"
  cidr_blocks       = ["0.0.0.0/0"]
  description       = "allow egress everywhere"
  security_group_id = aws_security_group.main.id
}

resource "aws_security_group_rule" "ingress_http" {
  type              = "ingress"
  from_port         = 80
  to_port           = 80
  protocol          = "tcp"
  cidr_blocks       = ["0.0.0.0/0"]
  description       = "allow HTTP connections"
  security_group_id = aws_security_group.main.id
}

resource "aws_security_group_rule" "ingress_https" {
  type              = "ingress"
  from_port         = 443
  to_port           = 443
  protocol          = "tcp"
  cidr_blocks       = ["0.0.0.0/0"]
  description       = "allow HTTPS connections"
  security_group_id = aws_security_group.main.id
}


#-------------------------------------------------------------------
# External ALB and DNS
#-------------------------------------------------------------------

module "external_label" {
  source = "github.com/Appreciate-Stuff/appr-tfmod-resource-label?ref=v1.1.0"

  attributes = ["ext"]
  context    = module.label.context
}

module "external_alb" {
  source = "github.com/Appreciate-Stuff/appr-tfmod-ec2-alb?ref=v0.4.0"

  context               = module.external_label.context
  subnet_ids            = module.legacy.vpcs[local.env].subnets.public
  vpc_id                = module.legacy.vpcs[local.env].id
  security_group_ids    = [aws_security_group.main.id]
  create_security_group = false
  is_internal           = false
}

resource "aws_lb_target_group" "external" {
  name         = "${module.external_label.id}-tg"
  target_type  = "ip"
  port         = local.container_port
  protocol     = "HTTP"
  vpc_id       = module.legacy.vpcs[local.env].id
  health_check {
    enabled             = true
    interval            = 30
    path                = "/health"
    port                = "traffic-port"
    healthy_threshold   = 2
    unhealthy_threshold = 2
    timeout             = 6
    protocol            = "HTTP"
    matcher             = "200,404"
  }
  tags = module.external_label.tags
}

resource "aws_lb_listener" "external_default" {
  load_balancer_arn = module.external_alb.alb_arn
  port              = "443"
  protocol          = "HTTPS"
  ssl_policy        = "ELBSecurityPolicy-2016-08"
  certificate_arn   = data.aws_acm_certificate.ssl_cert.arn

  default_action {
    type             = "forward"
    target_group_arn = aws_lb_target_group.external.arn
  }
}

resource "aws_lb_listener" "external_https_redirect" {
  load_balancer_arn = module.external_alb.alb_arn
  port              = "80"
  protocol          = "HTTP"

  default_action {
    type = "redirect"

    redirect {
      port        = "443"
      protocol    = "HTTPS"
      status_code = "HTTP_301"
    }
  }
}

resource "aws_route53_record" "external" {
  zone_id = module.legacy.dns[local.domain_name].public.zone_id
  name    = "${local.component}.${local.subdomain_names[local.env]}."
  type    = "CNAME"
  ttl     = 300
  records = [module.external_alb.alb_dns_name]
}

#-------------------------------------------------------------------
# External Pricing Virtualhost DNS
#-------------------------------------------------------------------

resource "aws_route53_record" "external_vhost_pricing" {
  zone_id = module.legacy.dns[local.domain_name].public.zone_id
  name    = "pricing.${local.subdomain_names[local.env]}."
  type    = "CNAME"
  ttl     = 300
  records = [module.external_alb.alb_dns_name]
}


#-------------------------------------------------------------------
# Internal ALB and DNS
#-------------------------------------------------------------------

module "internal_label" {
  source = "github.com/Appreciate-Stuff/appr-tfmod-resource-label?ref=v1.1.0"

  attributes = ["int"]
  context    = module.label.context
}

module "internal_alb" {
  source = "github.com/Appreciate-Stuff/appr-tfmod-ec2-alb?ref=v0.4.0"

  context               = module.internal_label.context
  subnet_ids            = module.legacy.vpcs[local.env].subnets.private
  vpc_id                = module.legacy.vpcs[local.env].id
  security_group_ids    = [aws_security_group.main.id]
  create_security_group = false
  is_internal           = true
}

resource "aws_lb_target_group" "internal" {
  name         = "${module.internal_label.id}-tg"
  target_type  = "ip"
  port         = local.container_port
  protocol     = "HTTP"
  vpc_id       = module.legacy.vpcs[local.env].id
  health_check {
    enabled             = true
    interval            = 30
    path                = "/health"
    port                = "traffic-port"
    healthy_threshold   = 2
    unhealthy_threshold = 2
    timeout             = 6
    protocol            = "HTTP"
    matcher             = "200,404"
  }
  tags = module.internal_label.tags
}

resource "aws_lb_listener" "internal_default" {
  load_balancer_arn = module.internal_alb.alb_arn
  port              = "443"
  protocol          = "HTTPS"
  ssl_policy        = "ELBSecurityPolicy-2016-08"
  certificate_arn   = data.aws_acm_certificate.ssl_cert.arn

  default_action {
    type             = "forward"
    target_group_arn = aws_lb_target_group.internal.arn
  }
}

resource "aws_lb_listener" "internal_https_redirect" {
  load_balancer_arn = module.internal_alb.alb_arn
  port              = "80"
  protocol          = "HTTP"

  default_action {
    type = "redirect"

    redirect {
      port        = "443"
      protocol    = "HTTPS"
      status_code = "HTTP_301"
    }
  }
}

resource "aws_route53_record" "internal" {
  zone_id = module.legacy.dns[local.domain_name].private.zone_id
  name    = "${local.component}.${local.subdomain_names[local.env]}."
  type    = "CNAME"
  ttl     = 300
  records = [module.internal_alb.alb_dns_name]
}

#-------------------------------------------------------------------
# User Actions - SQS
#-------------------------------------------------------------------

module "user_actions_queue" {
  source = "github.com/Appreciate-Stuff/appr-tfmod-state-core?ref=v0.1.3"

  service_path = "data/queues/user_actions"
  aws_region   = local.aws_region
  environment  = local.env
}

# This attaches the policy to our ECS role so that the apex service can access the queue
resource "aws_iam_role_policy_attachment" "user_actions_allow_sqs_queue_access" {
  role       = module.ecs_service.task_role_name
  policy_arn = module.user_actions_queue.service_state.full_access_policy.arn
}

#-------------------------------------------------------------------
# Service
#-------------------------------------------------------------------

data "terraform_remote_state" "ecr" {
  backend = "s3"

  config = {
    bucket         = var.state_bucket_name
    key            = "${var.state_bucket_root}/${var.project_name}/deploy/ecr/terraform.tfstate"
    region         = var.aws_state_region
    encrypt        = true
    dynamodb_table = join("--", [
      var.state_bucket_name,
      "${var.project_name}-${var.project_name}-deploy-ecr",
      "lock"
    ])
  }
}

locals {
  ecr_state  = data.terraform_remote_state.ecr.outputs
  account_id = module.legacy.aws_account_id

  # These secrets will be read from the SSM Parameter Store
  secret_path  = "/${local.env}/${local.component}/env"
  secret_names = [ "SENTRY_DSN", "HTTP_COOKIE_SECRET" ]
  secrets = [
    for secret_name in local.secret_names:
    {
      name      = secret_name
      valueFrom = "arn:aws:ssm:${var.aws_region}:${local.account_id}:parameter${local.secret_path}/${secret_name}"
    }
  ]

  # Only enable ECS Exec if we are in staging
  enable_ecs_exec = local.env == "stag"

  autoscaling_params = {
    stag = {
      tasks_desired_count           = 1
      min_count                     = 1
      max_count                     = 1
      tasks_minimum_healthy_percent = 50
      tasks_maximum_percent         = 200
      fargate_task_cpu              = 512
      fargate_task_memory           = 1024
      cpu                           = 256
      memory                        = 512
      wait_for_steady_state         = false
      scaling_target_value          = 40
      scale_in_cooldown             = 300
      scale_out_cooldown            = 300
    }
    prod = {
      tasks_desired_count           = 1
      min_count                     = 1
      max_count                     = 1
      tasks_minimum_healthy_percent = 50
      tasks_maximum_percent         = 200
      fargate_task_cpu              = 512
      fargate_task_memory           = 1024
      cpu                           = 256
      memory                        = 512
      wait_for_steady_state         = false
      scaling_target_value          = 40
      scale_in_cooldown             = 300
      scale_out_cooldown            = 300
    }
    thunderdome = {
      tasks_desired_count           = 1
      min_count                     = 1
      max_count                     = 1
      tasks_minimum_healthy_percent = 50
      tasks_maximum_percent         = 200
      fargate_task_cpu              = 512
      fargate_task_memory           = 1024
      cpu                           = 256
      memory                        = 512
      wait_for_steady_state         = false
      scaling_target_value          = 40
      scale_in_cooldown             = 300
      scale_out_cooldown            = 300
    }
  }
}

module "ecs_service" {
  source = "github.com/Appreciate-Stuff/appr-tfmod-ecs-service?ref=v0.1.1"

  enable          = "true"
  name            = local.component
  environment     = local.env
  ecs_use_fargate = true

  ecs_cluster_arn = module.legacy.ecs_clusters[local.env].arn
  cluster_name    = module.legacy.ecs_clusters[local.env].arn
  ecs_vpc_id      = module.legacy.vpcs[local.env].id
  ecs_subnet_ids  = module.legacy.vpcs[local.env].subnets.private

  enable_execute_command = local.enable_ecs_exec

  container_definitions  = templatefile("ecs_container_def.json.tpl",
    {
      name               = local.component
      image_name         = local.image_name
      image_tag          = local.image_tag
      container_port     = local.container_port
      account_id         = local.account_id
      ecr_repo           = "${local.account_id}.dkr.ecr.${local.aws_region}.amazonaws.com"
      aws_region         = local.aws_region
      env                = local.env
      cloudwatchlog_name = "/ecs/${local.env}/${local.env}"
      secrets_json       = jsonencode(local.secrets)
      cpu                = local.autoscaling_params[local.env].cpu
      memory             = local.autoscaling_params[local.env].memory
      user_actions_url   = module.user_actions_queue.service_state.sqs_queue.id

      init_process_enabled_json = jsonencode(local.enable_ecs_exec)
    }
  )

  wait_for_steady_state         = local.autoscaling_params[local.env].wait_for_steady_state
  tasks_desired_count           = local.autoscaling_params[local.env].tasks_desired_count
  min_count                     = local.autoscaling_params[local.env].min_count
  max_count                     = local.autoscaling_params[local.env].max_count
  tasks_minimum_healthy_percent = local.autoscaling_params[local.env].tasks_minimum_healthy_percent
  tasks_maximum_percent         = local.autoscaling_params[local.env].tasks_maximum_percent
  fargate_task_cpu              = local.autoscaling_params[local.env].fargate_task_cpu
  fargate_task_memory           = local.autoscaling_params[local.env].fargate_task_memory
  scale_up_cooldown_seconds     = local.autoscaling_params[local.env].scale_out_cooldown
  scale_down_cooldown_seconds   = local.autoscaling_params[local.env].scale_in_cooldown
  scaling_target_value          = local.autoscaling_params[local.env].scaling_target_value
  ecs_task_def_network_mode     = "awsvpc"
  associate_alb                 = true
  container_port                = local.container_port
  alb_security_group            = aws_security_group.main.id

  lb_target_group_arn = {
    config = [
      {
        target_group_arn = aws_lb_target_group.external.arn
        container_port   = local.container_port
      },
      {
        target_group_arn = aws_lb_target_group.internal.arn
        container_port   = local.container_port
      }
    ]
  }

  tags = module.label.tags
}
