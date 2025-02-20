module "eks" {
  source  = "terraform-aws-modules/eks/aws"
  version = "~> 20.31"

  cluster_endpoint_public_access = true
  enable_cluster_creator_admin_permissions = true

  cluster_name    = local.name
  cluster_version = "1.32"

  # EKS Addons
  cluster_addons = {
    coredns                = {}
    eks-pod-identity-agent = {}
    kube-proxy             = {}
    vpc-cni                = {}
    aws-ebs-csi-driver     = {}
  }

  vpc_id     = module.vpc.vpc_id
  subnet_ids = module.vpc.private_subnets

  eks_managed_node_groups = {
    default_nodes = {
      instance_types = ["t3.medium"]

      min_size = 1
      max_size = 4
      # This value is ignored after the initial creation
      # https://github.com/bryantbiggs/eks-desired-size-hack
      desired_size = 4

      iam_role_additional_policies = {
        AmazonEKS_CNI_Policy = "arn:aws:iam::aws:policy/AmazonEKS_CNI_Policy"
        SecretsManagerReadWrite = "arn:aws:iam::aws:policy/SecretsManagerReadWrite"
        AmazonRDSFullAccess = "arn:aws:iam::aws:policy/AmazonRDSFullAccess"
        AmazonSQSFullAccess = "arn:aws:iam::aws:policy/AmazonSQSFullAccess"
        AmazonEC2FullAccess = "arn:aws:iam::aws:policy/AmazonEC2FullAccess"
      }
    }
  }

  tags = local.tags
}
