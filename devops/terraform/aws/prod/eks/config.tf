terraform {
  backend "s3" {
    bucket         = "intuition-terraform-state"
    key            = "terraform/state/prod/eks-vpc/terraform.state"
    region         = "us-west-2"
    encrypt        = true
  }
}

