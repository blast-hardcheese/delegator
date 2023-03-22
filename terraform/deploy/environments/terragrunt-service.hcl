include "root" {
  path   = find_in_parent_folders()
  expose = true
}

dependencies {
  paths = [
    "${path_relative_from_include()}/deploy/ecr",
  ]
}

locals {
  aws_region = basename(dirname(get_terragrunt_dir()))
  env        = basename(dirname(dirname(get_terragrunt_dir())))
  image_tag  = get_env("IMAGE_TAG")
  namespace  = "app"
  product    = "platform"
  component  = include.root.locals.project_name
}

inputs = {
  aws_region = local.aws_region
  env        = local.env
  image_tag  = local.image_tag
  namespace  = local.namespace
  product    = local.product
  component  = local.component

  state_bucket_name = include.root.locals.state_bucket_name
  state_bucket_root = include.root.locals.state_bucket_root
  project_name      = include.root.locals.project_name
  project_path      = include.root.locals.project_path
  repository_root   = include.root.locals.repository_root
  aws_state_region  = include.root.locals.aws_state_region
  state_lock_table  = include.root.locals.state_lock_table

}

terraform {
  source = "${get_parent_terragrunt_dir()}/modules//envconfig"
}
