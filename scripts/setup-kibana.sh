#!/bin/bash

# Setup Kibana with pre-configured searches for Intuition system monitoring

echo "Setting up Kibana with pre-configured searches..."

# Wait for Kibana to be ready
echo "Waiting for Kibana to be ready..."
until curl -f http://localhost:5601/api/status > /dev/null 2>&1; do
    echo "Waiting for Kibana..."
    sleep 5
done

echo "Kibana is ready! Setting up saved objects..."

# Create saved objects for common log searches
curl -X POST "http://localhost:5601/api/saved_objects/_import" \
  -H "kbn-xsrf: true" \
  -H "Content-Type: application/json" \
  --data-binary '{
    "version": "8.12.0",
    "objects": [
      {
        "id": "decoded-consumer-logs",
        "type": "search",
        "attributes": {
          "title": "Decoded Consumer INFO Logs",
          "description": "Filter logs from decoded consumer with INFO level",
          "kibanaSavedObjectMeta": {
            "searchSourceJSON": "{\"query\":{\"bool\":{\"must\":[{\"match\":{\"service\":\"decoded_consumer\"}},{\"match\":{\"level\":\"INFO\"}}]}},\"index\":\"docker-logs-*\"}"
          }
        }
      },
      {
        "id": "resolver-consumer-logs",
        "type": "search",
        "attributes": {
          "title": "Resolver Consumer INFO Logs",
          "description": "Filter logs from resolver consumer with INFO level",
          "kibanaSavedObjectMeta": {
            "searchSourceJSON": "{\"query\":{\"bool\":{\"must\":[{\"match\":{\"service\":\"resolver_consumer\"}},{\"match\":{\"level\":\"INFO\"}}]}},\"index\":\"docker-logs-*\"}"
          }
        }
      },
      {
        "id": "ipfs-upload-consumer-logs",
        "type": "search",
        "attributes": {
          "title": "IPFS Upload Consumer INFO Logs",
          "description": "Filter logs from IPFS upload consumer with INFO level",
          "kibanaSavedObjectMeta": {
            "searchSourceJSON": "{\"query\":{\"bool\":{\"must\":[{\"match\":{\"service\":\"ipfs_upload_consumer\"}},{\"match\":{\"level\":\"INFO\"}}]}},\"index\":\"docker-logs-*\"}"
          }
        }
      },
      {
        "id": "histocrawler-logs",
        "type": "search",
        "attributes": {
          "title": "Histocrawler INFO Logs",
          "description": "Filter logs from histocrawler with INFO level",
          "kibanaSavedObjectMeta": {
            "searchSourceJSON": "{\"query\":{\"bool\":{\"must\":[{\"match\":{\"service\":\"histocrawler\"}},{\"match\":{\"level\":\"INFO\"}}]}},\"index\":\"docker-logs-*\"}"
          }
        }
      },
      {
        "id": "all-consumers-logs",
        "type": "search",
        "attributes": {
          "title": "All Consumers INFO Logs",
          "description": "Combined view of all consumer logs with INFO level",
          "kibanaSavedObjectMeta": {
            "searchSourceJSON": "{\"query\":{\"bool\":{\"must\":[{\"match\":{\"level\":\"INFO\"}},{\"bool\":{\"should\":[{\"match\":{\"service\":\"decoded_consumer\"}},{\"match\":{\"service\":\"resolver_consumer\"}},{\"match\":{\"service\":\"ipfs_upload_consumer\"}},{\"match\":{\"service\":\"histocrawler\"}}]}}]}},\"index\":\"docker-logs-*\"}"
          }
        }
      }
    ]
  }' > /dev/null 2>&1

if [ $? -eq 0 ]; then
    echo "✅ Kibana setup completed successfully!"
    echo "📊 Access Kibana at: http://localhost:5601"
    echo "🔍 Pre-configured searches are now available in the 'Saved Objects' section"
else
    echo "⚠️  Kibana setup completed with warnings. You may need to manually create saved objects."
    echo "📊 Access Kibana at: http://localhost:5601"
fi

echo ""
echo "🎯 Quick access to common log searches:"
echo "   - Decoded Consumer INFO Logs"
echo "   - Resolver Consumer INFO Logs" 
echo "   - IPFS Upload Consumer INFO Logs"
echo "   - Histocrawler INFO Logs"
echo "   - All Consumers INFO Logs (combined view)"
echo ""
echo "💡 Tip: Use these saved searches to quickly monitor your Intuition system!"
