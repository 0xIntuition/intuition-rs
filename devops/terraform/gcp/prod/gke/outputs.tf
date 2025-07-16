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

output "private_subnet" {
  description = "Private subnet name"
  value       = google_compute_subnetwork.private_subnet.name
}

output "private_subnet_id" {
  description = "Private subnet ID"
  value       = google_compute_subnetwork.private_subnet.id
}

output "project_id" {
  description = "GCP project ID"
  value       = var.project_id
}

output "region" {
  description = "GCP region"
  value       = var.region
} 