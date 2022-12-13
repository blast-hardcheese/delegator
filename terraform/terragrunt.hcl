terraform_version_constraint = local.terraform_version
terragrunt_version_constraint = local.terragrunt_version

locals {
  terraform_version  = "~> 1.3.2"
  terragrunt_version = "~> 0.39.1"

  aws_state_region  = "us-east-1"
  state_bucket_name = "appr-terraform-app-state"
  state_bucket_root = "app/"
  project_name      = basename(dirname(get_parent_terragrunt_dir()))
  project_path      = "${local.project_name}/${path_relative_to_include()}"
  state_lock_table  = join("--", [
    "${local.state_bucket_name}",
    replace(local.project_path, "/", "-"),
    "lock",
  ])
  repository_root   = path_relative_from_include()
}

remote_state {
  generate = {
    path      = "generated_backend.tf"
    if_exists = "overwrite_terragrunt"
  }

  backend = "s3"
  config = {
    bucket         = local.state_bucket_name
    key            = "${local.state_bucket_root}/${local.project_path}/terraform.tfstate"
    region         = local.aws_state_region
    encrypt        = true
    dynamodb_table = local.state_lock_table 
    access_key     = get_env("AWS_ACCESS_KEY_ID", "")
    secret_key     = get_env("AWS_SECRET_ACCESS_KEY", "")
    profile        = get_env("AWS_PROFILE", "")
  }
}

generate "provider" {
  path      = "generated_provider.tf"
  if_exists = "overwrite"
  contents = <<EOF
terraform {
  required_version = "${local.terraform_version}"

  required_providers {
    aws = {
      source = "hashicorp/aws"
      version = "~> 4.0"
    }
  }
}
provider "aws" {
  region = "${local.aws_state_region}"
}
EOF
}

terraform {
  before_hook "update_modules" {
    commands = ["plan", "apply", "import", "refresh"]
    execute = ["terraform", "get", "-update"]
  }

  extra_arguments "no_plan_lock" {
    commands = ["plan"]
    arguments = ["-lock=false"]
  }

  after_hook "rm_generated" {
    commands = ["apply","console","destroy","env","fmt","get","graph","import","init","output","plan","refresh","show","taint","untaint","validate","workspace"]
    execute = [
      "rm",
      "-f",
      "${get_terragrunt_dir()}/generated_backend.tf",
      "${get_terragrunt_dir()}/generated_provider.tf",
    ]
    run_on_error = true
  }
}
