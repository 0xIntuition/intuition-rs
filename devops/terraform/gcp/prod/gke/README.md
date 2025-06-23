# GCP GKE Terraform Configuration

This Terraform configuration creates a production-ready GKE (Google Kubernetes Engine) cluster with equivalent functionality to the AWS EKS setup.

## Features

- **VPC Network**: Custom VPC with private and public subnets across 3 availability zones
- **GKE Cluster**: Private cluster with Workload Identity enabled
- **Node Pools**: 
  - Default pool (e2-medium, equivalent to t3.medium)
  - Large pool (e2-standard-4, equivalent to m5.xlarge) with preemptible instances
- **Security**: Network policies, secure boot, private nodes
- **Monitoring**: Managed Prometheus and logging enabled
- **Networking**: Cloud NAT for private subnet internet access

## Prerequisites

1. **GCP Project**: Create a GCP project and enable required APIs:
   ```bash
   gcloud services enable container.googleapis.com
   gcloud services enable compute.googleapis.com
   gcloud services enable monitoring.googleapis.com
   gcloud services enable logging.googleapis.com
   ```

2. **GCS Bucket**: Create a GCS bucket for Terraform state:
   ```bash
   gsutil mb gs://intuition-terraform-state-gcp
   gsutil versioning set on gs://intuition-terraform-state-gcp
   ```

3. **Service Account**: Create a service account with required permissions:
   ```bash
   gcloud iam service-accounts create terraform-sa \
     --display-name="Terraform Service Account"
   
   gcloud projects add-iam-policy-binding YOUR_PROJECT_ID \
     --member="serviceAccount:terraform-sa@YOUR_PROJECT_ID.iam.gserviceaccount.com" \
     --role="roles/editor"
   
   gcloud iam service-accounts keys create terraform-key.json \
     --iam-account=terraform-sa@YOUR_PROJECT_ID.iam.gserviceaccount.com
   ```

## Configuration

1. **Update Project ID**: Replace `your-gcp-project-id` in `main.tf` and `variables.tf` with your actual GCP project ID.

2. **Set Environment Variables**:
   ```bash
   export GOOGLE_PROJECT="your-gcp-project-id"
   export GOOGLE_APPLICATION_CREDENTIALS="path/to/terraform-key.json"
   ```

3. **Customize Variables** (optional): Create a `terraform.tfvars` file:
   ```hcl
   project_id = "your-gcp-project-id"
   region     = "us-west2"
   cluster_name = "prod-gke"
   ```

## Usage

```bash
# Initialize Terraform
terraform init

# Plan the deployment
terraform plan

# Apply the configuration
terraform apply

# Get cluster credentials
gcloud container clusters get-credentials prod-gke --region us-west2 --project your-gcp-project-id
```

## Key Differences from AWS EKS

| AWS EKS | GCP GKE |
|---------|---------|
| EKS Cluster | GKE Cluster |
| VPC with NAT Gateway | VPC with Cloud NAT |
| IAM Roles | Workload Identity |
| Spot Instances | Preemptible Instances |
| EBS CSI Driver | GCE Persistent Disk CSI Driver |
| CloudWatch | Cloud Monitoring/Logging |

## Security Features

- **Private Cluster**: Nodes run in private subnets
- **Workload Identity**: Secure pod-to-service authentication
- **Network Policies**: Pod-level network security
- **Secure Boot**: Hardware-level security
- **Master Authorized Networks**: Control access to cluster control plane

## Cost Optimization

- **Preemptible Instances**: Used for large node pool (equivalent to AWS spot instances)
- **Regional Clusters**: Single control plane across multiple zones
- **Autoscaling**: Node pools scale based on demand

## Monitoring and Logging

- **Managed Prometheus**: Built-in monitoring
- **Cloud Logging**: Centralized logging
- **Flow Logs**: Network traffic monitoring

## Outputs

After deployment, you'll get:
- Cluster endpoint and credentials
- VPC and subnet information
- Node pool details
- Project and region information

## Cleanup

```bash
terraform destroy
```

**Warning**: This will delete all resources including the GKE cluster and VPC. 