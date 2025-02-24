#!/bin/bash
geth init /config/genesis.json
geth "$@"
