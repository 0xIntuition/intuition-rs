#!/bin/bash

# This script applies the metadata fix for the share_price_change conflict

# Export the current metadata
hasura metadata export

# Apply the metadata changes
hasura metadata apply

# Reload the metadata
hasura metadata reload 