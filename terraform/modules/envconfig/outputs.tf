output "external_alb" {
  value = {
    arn = module.external_alb.alb_arn
    dns_name = module.external_alb.alb_dns_name
    log_bucket = module.external_alb.alb_log_bucket
  }
}

output "external_alb_security_group" {
  value = {
    id = aws_security_group.main.id
    arn = aws_security_group.main.arn
    name = aws_security_group.main.name
  }
}
