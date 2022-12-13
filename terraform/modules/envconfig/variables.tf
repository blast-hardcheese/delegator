variable "aws_region" {
  type = string
}
variable "env" {
  type = string
}
variable "image_tag" {
  type = string
  validation {
    condition     = length(var.image_tag) > 0 && var.image_tag != null
    error_message = "Docker image tag must be provided and cannot be an empty string!"
  }
}
variable "state_bucket_name" {
  type = string
}
variable "state_bucket_root" {
  type = string
}
variable "project_name" {
  type = string
}
variable "project_path" {
  type = string
}
variable "repository_root" {
  type = string
}
variable "state_lock_table" {
  type = string
}
variable "aws_state_region" {
  type = string
}
