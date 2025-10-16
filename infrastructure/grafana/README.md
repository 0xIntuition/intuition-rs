# Grafana Setup

## Access Grafana

- **URL**: http://localhost:3001
- **Username**: `admin`
- **Password**: `admin`

## Datasource

Prometheus datasource is auto-provisioned and connected to `http://prometheus:9090`.

## Import Dashboard

To import the Decoded Consumer Performance dashboard:

1. Go to http://localhost:3001
2. Login with `admin/admin`
3. Click the "+" icon or "Dashboards" in the left menu
4. Click "Import"
5. Click "Upload JSON file"
6. Select `/infrastructure/grafana/dashboards/decoded-consumer-performance.json`
7. Select "Prometheus" as the datasource
8. Click "Import"

The dashboard is now available and will show metrics from your consumer service.

