#!/bin/bash

echo "Waiting for Kibana to be ready..."
until curl -s http://localhost:5601/api/status > /dev/null; do
    sleep 5
done

echo "Creating index pattern..."
curl -X POST "http://localhost:5601/api/data_views/data_view" \
  -H "kbn-xsrf: true" \
  -H "Content-Type: application/json" \
  -d '{
    "data_view": {
      "title": "docker-logs-*",
      "timeFieldName": "@timestamp"
    }
  }'

echo "Importing saved objects to Kibana..."
curl -X POST "http://localhost:5601/api/saved_objects/_import?overwrite=true" \
  -H "kbn-xsrf: true" \
  -H "Content-Type: multipart/form-data" \
  -F "file=@elk/kibana/saved_objects.json"

echo "Kibana setup complete!"
echo "Visit http://localhost:5601/app/discover"
echo ""
echo "Available Saved Searches:"
echo "- Decoded Consumer INFO Logs"
echo "- Resolver Consumer INFO Logs" 
echo "- IPFS Upload Consumer INFO Logs"
echo "- Histocrawler INFO Logs"
echo "- All Consumers INFO Logs (combined view)"
echo ""
echo "You can also use these filters directly:"
echo "- service:decoded_consumer AND level:INFO"
echo "- service:resolver_consumer AND level:INFO"
echo "- service:ipfs_upload_consumer AND level:INFO"
echo "- service:histocrawler AND level:INFO"
