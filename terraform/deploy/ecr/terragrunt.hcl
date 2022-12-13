include "root" {
  path   = find_in_parent_folders()
  expose = true
}

locals { 
  aws_region = "us-east-1"
  namespace  = "app"
  product    = "platform"
}

inputs = {
  namespace  = local.namespace
  product    = local.product
  component  = include.root.locals.project_name
  aws_region = local.aws_region
}

terraform {
  source = "github.com/Appreciate-Stuff/appr-tfmod-ecr-repository//"
}
