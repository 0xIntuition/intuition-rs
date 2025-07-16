provider "google" {
  project = var.project_id
  region  = var.region
}

variable "project_id" {
  type    = string
  default = "be-cluster"
}

variable "region" {
  type    = string
  default = "us-west2"
}

# VPC
resource "google_compute_network" "vpc" {
  name                    = "debug-vpc"
  auto_create_subnetworks = false
}

# Subnets
resource "google_compute_subnetwork" "private_subnet" {
  name                     = "debug-private-subnet"
  region                   = var.region
  network                  = google_compute_network.vpc.id
  ip_cidr_range            = "10.10.0.0/24"
  private_ip_google_access = true
}

resource "google_compute_subnetwork" "public_subnet" {
  name                     = "debug-public-subnet"
  region                   = var.region
  network                  = google_compute_network.vpc.id
  ip_cidr_range            = "10.20.0.0/24"
  private_ip_google_access = false
}

# Router + NAT
resource "google_compute_router" "router" {
  name    = "debug-router"
  region  = var.region
  network = google_compute_network.vpc.id
}

resource "google_compute_router_nat" "nat" {
  name                               = "debug-nat"
  router                             = google_compute_router.router.name
  region                             = var.region
  nat_ip_allocate_option             = "AUTO_ONLY"
  source_subnetwork_ip_ranges_to_nat = "LIST_OF_SUBNETWORKS"

  subnetwork {
    name                    = google_compute_subnetwork.private_subnet.name
    source_ip_ranges_to_nat = ["ALL_IP_RANGES"]
  }
}

# GKE Cluster
resource "google_container_cluster" "primary" {
  name     = "debug-cluster"
  location = var.region

  remove_default_node_pool = true
  initial_node_count       = 1
  deletion_protection = false

  network    = google_compute_network.vpc.name
  subnetwork = google_compute_subnetwork.private_subnet.self_link

  ip_allocation_policy {}

  private_cluster_config {
    enable_private_nodes    = true
    enable_private_endpoint = false
    master_ipv4_cidr_block  = "172.31.0.16/28"
  }

  workload_identity_config {
    workload_pool = "${var.project_id}.svc.id.goog"
  }

  release_channel {
    channel = "REGULAR"
  }
}

# Node Pool: DB
resource "google_container_node_pool" "db_pool" {
  name     = "db-pool"
  cluster  = google_container_cluster.primary.name
  location = var.region

  node_config {
    machine_type = "n2-standard-16"
    oauth_scopes = ["https://www.googleapis.com/auth/cloud-platform"]

    labels = {
      role = "db"
    }

    taint {
      key    = "db"
      value  = "true"
      effect = "NO_SCHEDULE"
    }

    metadata = {
      disable-legacy-endpoints = "true"
    }

    shielded_instance_config {
      enable_secure_boot = true
    }

    workload_metadata_config {
      mode = "GKE_METADATA"
    }
  }

  initial_node_count = 1
}

# Node Pool: App Services
resource "google_container_node_pool" "app_pool" {
  name     = "app-pool"
  cluster  = google_container_cluster.primary.name
  location = var.region

  node_config {
    machine_type = "e2-standard-2"
    oauth_scopes = ["https://www.googleapis.com/auth/cloud-platform"]

    labels = {
      role = "app"
    }

    metadata = {
      disable-legacy-endpoints = "true"
    }

    shielded_instance_config {
      enable_secure_boot = true
    }

    workload_metadata_config {
      mode = "GKE_METADATA"
    }
  }

  initial_node_count = 5
}

# Node Pool: Consumer
resource "google_container_node_pool" "consumer_pool" {
  name     = "consumer-pool"
  cluster  = google_container_cluster.primary.name
  location = var.region

  node_config {
    machine_type = "custom-4-8192"
    oauth_scopes = ["https://www.googleapis.com/auth/cloud-platform"]

    labels = {
      role = "consumer"
    }

    taint {
      key    = "consumer"
      value  = "true"
      effect = "NO_SCHEDULE"
    }

    metadata = {
      disable-legacy-endpoints = "true"
    }

    shielded_instance_config {
      enable_secure_boot = true
    }

    workload_metadata_config {
      mode = "GKE_METADATA"
    }
  }

  initial_node_count = 1
}
