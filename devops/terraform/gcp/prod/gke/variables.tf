variable "project_id" {
  description = "GCP project ID"
  type        = string
  default     = "your-gcp-project-id" # Replace with your actual project ID
}

variable "region" {
  description = "GCP region"
  type        = string
  default     = "us-west2"
}

variable "cluster_name" {
  description = "GKE cluster name"
  type        = string
  default     = "prod-gke"
}

variable "environment" {
  description = "Environment name"
  type        = string
  default     = "prod"
}

variable "vpc_cidr" {
  description = "VPC CIDR block"
  type        = string
  default     = "10.0.0.0/16"
}

variable "private_subnet_cidrs" {
  description = "Private subnet CIDR blocks"
  type        = list(string)
  default     = ["10.0.1.0/24", "10.0.2.0/24", "10.0.3.0/24"]
}

variable "public_subnet_cidrs" {
  description = "Public subnet CIDR blocks"
  type        = list(string)
  default     = ["10.0.101.0/24", "10.0.102.0/24", "10.0.103.0/24"]
}

variable "default_node_pool_config" {
  description = "Default node pool configuration"
  type = object({
    machine_type  = string
    disk_size_gb  = number
    min_nodes     = number
    max_nodes     = number
    initial_nodes = number
  })
  default = {
    machine_type  = "e2-medium"
    disk_size_gb  = 100
    min_nodes     = 1
    max_nodes     = 6
    initial_nodes = 6
  }
}

variable "large_node_pool_config" {
  description = "Large node pool configuration"
  type = object({
    machine_type  = string
    disk_size_gb  = number
    min_nodes     = number
    max_nodes     = number
    initial_nodes = number
    preemptible   = bool
  })
  default = {
    machine_type  = "e2-standard-4"
    disk_size_gb  = 100
    min_nodes     = 1
    max_nodes     = 2
    initial_nodes = 2
    preemptible   = true
  }
}

variable "enable_private_cluster" {
  description = "Enable private GKE cluster"
  type        = bool
  default     = true
}

variable "enable_workload_identity" {
  description = "Enable Workload Identity"
  type        = bool
  default     = true
}

variable "enable_network_policy" {
  description = "Enable network policy"
  type        = bool
  default     = true
}

variable "enable_managed_prometheus" {
  description = "Enable Managed Prometheus"
  type        = bool
  default     = true
} 