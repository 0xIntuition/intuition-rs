provider "google" {
  project = local.project_id
  region  = local.region
}

locals {
  name       = "prod-gke"
  project_id = "your-gcp-project-id"  # Replace with your actual project ID
  region     = "us-west2"
  env        = "prod"

  vpc_cidr = "10.0.0.0/16"
  subnet_cidrs = {
    private = ["10.0.1.0/24", "10.0.2.0/24", "10.0.3.0/24"]
    public  = ["10.0.101.0/24", "10.0.102.0/24", "10.0.103.0/24"]
  }

  tags = {
    Name = local.name
    Env  = local.env
  }
}

################################################################################
# VPC Network
################################################################################

resource "google_compute_network" "vpc" {
  name                    = local.name
  auto_create_subnetworks = false
  routing_mode           = "REGIONAL"
}

resource "google_compute_subnetwork" "private" {
  count         = length(local.subnet_cidrs.private)
  name          = "${local.name}-private-${count.index + 1}"
  ip_cidr_range = local.subnet_cidrs.private[count.index]
  region        = local.region
  network       = google_compute_network.vpc.id

  # Enable flow logs for better network visibility
  log_config {
    aggregation_interval = "INTERVAL_5_SEC"
    flow_sampling       = 0.5
    metadata           = "INCLUDE_ALL_METADATA"
  }

  # Enable private Google access for GKE nodes
  private_ip_google_access = true
}

resource "google_compute_subnetwork" "public" {
  count         = length(local.subnet_cidrs.public)
  name          = "${local.name}-public-${count.index + 1}"
  ip_cidr_range = local.subnet_cidrs.public[count.index]
  region        = local.region
  network       = google_compute_network.vpc.id

  log_config {
    aggregation_interval = "INTERVAL_5_SEC"
    flow_sampling       = 0.5
    metadata           = "INCLUDE_ALL_METADATA"
  }
}

################################################################################
# Cloud NAT for private subnets
################################################################################

resource "google_compute_router" "router" {
  name    = "${local.name}-router"
  region  = local.region
  network = google_compute_network.vpc.id
}

resource "google_compute_router_nat" "nat" {
  name                               = "${local.name}-nat"
  router                            = google_compute_router.router.name
  region                            = local.region
  nat_ip_allocate_option            = "AUTO_ONLY"
  source_subnetwork_ip_ranges_to_nat = "ALL_SUBNETWORKS_ALL_IP_RANGES"
}

################################################################################
# GKE Cluster
################################################################################

resource "google_container_cluster" "primary" {
  name     = local.name
  location = local.region

  # Remove default node pool
  remove_default_node_pool = true
  initial_node_count       = 1

  network    = google_compute_network.vpc.name
  subnetwork = google_compute_subnetwork.private[0].name

  # Enable Workload Identity for better security
  workload_pool_config {
    workload_pool = "${local.project_id}.svc.id.goog"
  }

  # Enable IP aliasing for better networking
  ip_allocation_policy {
    cluster_ipv4_cidr_block  = "/16"
    services_ipv4_cidr_block = "/22"
  }

  # Enable private cluster
  private_cluster_config {
    enable_private_nodes    = true
    enable_private_endpoint = false
    master_ipv4_cidr_block  = "172.16.0.0/28"
  }

  # Enable network policy
  network_policy {
    enabled = true
  }

  # Enable master authorized networks for secure access
  master_authorized_networks_config {
    cidr_blocks {
      cidr_block   = "0.0.0.0/0"
      display_name = "All"
    }
  }

  # Enable release channel for automatic updates
  release_channel {
    channel = "REGULAR"
  }

  # Enable maintenance windows
  maintenance_policy {
    recurring_window {
      start_time = "2024-01-01T02:00:00Z"
      end_time   = "2024-01-01T06:00:00Z"
      recurrence = "FREQ=WEEKLY;BYDAY=SU"
    }
  }

  # Enable node auto-upgrade
  node_config {
    machine_type = "e2-medium"
    disk_size_gb = 100

    # Enable workload identity
    workload_metadata_config {
      mode = "GKE_METADATA"
    }

    # Enable secure boot
    shielded_instance_config {
      enable_secure_boot = true
    }

    # Enable confidential nodes (optional, for additional security)
    # confidential_nodes {
    #   enabled = true
    # }

    metadata = {
      disable-legacy-endpoints = "true"
    }

    oauth_scopes = [
      "https://www.googleapis.com/auth/logging.write",
      "https://www.googleapis.com/auth/monitoring",
      "https://www.googleapis.com/auth/devstorage.read_only",
      "https://www.googleapis.com/auth/cloud-platform"
    ]

    labels = local.tags
  }

  # Enable addons
  addons_config {
    http_load_balancing {
      disabled = false
    }
    horizontal_pod_autoscaling {
      disabled = false
    }
    network_policy_config {
      disabled = false
    }
    gce_persistent_disk_csi_driver_config {
      enabled = true
    }
  }

  # Enable monitoring
  monitoring_config {
    enable_components = ["SYSTEM_COMPONENTS", "WORKLOADS"]
    managed_prometheus {
      enabled = true
    }
  }

  # Enable logging
  logging_config {
    enable_components = ["SYSTEM_COMPONENTS", "WORKLOADS"]
  }

  resource_labels = local.tags
}

################################################################################
# Node Pools
################################################################################

# Default node pool (equivalent to t3.medium)
resource "google_container_node_pool" "default_nodes" {
  name       = "${local.name}-default-nodes"
  location   = local.region
  cluster    = google_container_cluster.primary.name
  node_count = 6

  autoscaling {
    min_node_count = 1
    max_node_count = 6
  }

  node_config {
    machine_type = "e2-medium"
    disk_size_gb = 100

    # Enable workload identity
    workload_metadata_config {
      mode = "GKE_METADATA"
    }

    # Enable secure boot
    shielded_instance_config {
      enable_secure_boot = true
    }

    metadata = {
      disable-legacy-endpoints = "true"
    }

    oauth_scopes = [
      "https://www.googleapis.com/auth/logging.write",
      "https://www.googleapis.com/auth/monitoring",
      "https://www.googleapis.com/auth/devstorage.read_only",
      "https://www.googleapis.com/auth/cloud-platform"
    ]

    labels = merge(local.tags, {
      "node-pool" = "default"
    })

    taint {
      key    = "node-pool"
      value  = "default"
      effect = "NO_SCHEDULE"
    }
  }

  management {
    auto_repair  = true
    auto_upgrade = true
  }

  upgrade_settings {
    max_surge       = 1
    max_unavailable = 0
  }
}

# Large node pool (equivalent to m5.xlarge)
resource "google_container_node_pool" "large_nodes" {
  name       = "${local.name}-large-nodes"
  location   = local.region
  cluster    = google_container_cluster.primary.name
  node_count = 2

  autoscaling {
    min_node_count = 1
    max_node_count = 2
  }

  node_config {
    machine_type = "e2-standard-4"  # Equivalent to m5.xlarge
    disk_size_gb = 100

    # Enable workload identity
    workload_metadata_config {
      mode = "GKE_METADATA"
    }

    # Enable secure boot
    shielded_instance_config {
      enable_secure_boot = true
    }

    # Use preemptible instances for cost savings (equivalent to spot instances)
    preemptible = true

    metadata = {
      disable-legacy-endpoints = "true"
    }

    oauth_scopes = [
      "https://www.googleapis.com/auth/logging.write",
      "https://www.googleapis.com/auth/monitoring",
      "https://www.googleapis.com/auth/devstorage.read_only",
      "https://www.googleapis.com/auth/cloud-platform"
    ]

    labels = merge(local.tags, {
      "node-pool" = "large"
      "spot"      = "true"
    })

    taint {
      key    = "node-pool"
      value  = "large"
      effect = "NO_SCHEDULE"
    }
  }

  management {
    auto_repair  = true
    auto_upgrade = true
  }

  upgrade_settings {
    max_surge       = 1
    max_unavailable = 0
  }
} 