output "cluster_name" {
  description = "GKE cluster name"
  value       = google_container_cluster.primary.name
}

output "cluster_endpoint" {
  description = "GKE cluster endpoint"
  value       = google_container_cluster.primary.endpoint
}

output "cluster_location" {
  description = "GKE cluster location"
  value       = google_container_cluster.primary.location
}

output "cluster_master_version" {
  description = "GKE cluster master version"
  value       = google_container_cluster.primary.master_version
}

output "vpc_name" {
  description = "VPC name"
  value       = google_compute_network.vpc.name
}

output "vpc_id" {
  description = "VPC ID"
  value       = google_compute_network.vpc.id
}

output "private_subnets" {
  description = "Private subnet names"
  value       = google_compute_subnetwork.private[*].name
}

output "public_subnets" {
  description = "Public subnet names"
  value       = google_compute_subnetwork.public[*].name
}

output "node_pools" {
  description = "Node pool names"
  value = {
    default = google_container_node_pool.default_nodes.name
    large   = google_container_node_pool.large_nodes.name
  }
}

output "project_id" {
  description = "GCP project ID"
  value       = local.project_id
}

output "region" {
  description = "GCP region"
  value       = local.region
} 