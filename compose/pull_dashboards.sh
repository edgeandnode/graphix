#!/bin/bash

# Run this script from `compose/` while Grafana is running to automatically export all dashboards.

# API token from Grafana (view-only service account, configured in the grafana.db)
API_TOKEN="eyJrIjoiUE1nZU1WQXdrdlphZUhlanVLR012eUhkZGxSNEJtMTUiLCJuIjoiYWRtaW4iLCJpZCI6MX0="

# Grafana URL (adjust as needed)
GRAFANA_URL="http://localhost:3000"

# Directory to save dashboards
DASHBOARD_DIR="../grafana/dashboards"

# Create directory if it doesn't exist
mkdir -p $DASHBOARD_DIR

# Get list of all dashboards
DASHBOARD_UIDS=$(curl -s --fail -H "Authorization: Bearer $API_TOKEN" "$GRAFANA_URL/api/search" | jq -r '.[].uid')

# Iterate through UIDs and download each dashboard
COUNTER=0
for DASHBOARD_UID in $DASHBOARD_UIDS; do
  DASHBOARD_JSON=$(curl --fail -s -H "Authorization: Bearer $API_TOKEN" "$GRAFANA_URL/api/dashboards/uid/$DASHBOARD_UID" | jq '.dashboard.id = null' | jq '.dashboard')
  TITLE=$(echo $DASHBOARD_JSON | jq -r '.title')

  # Replace spaces with underscores
  FILENAME=$(echo "$TITLE.json" | tr " " "_")

  # Check if the title is null, empty, or only whitespace
  if [ -z "$TITLE" ] || [ "$TITLE" == "null" ] || [ "$FILENAME" == ".json" ]; then
    echo "Skipping dashboard with UID $DASHBOARD_UID - No valid title found"
    continue
  fi

  FILEPATH="$DASHBOARD_DIR/$FILENAME"

  # If the file already exists, compare the entire JSON except the 'version' field
  if [ -f "$FILEPATH" ]; then
    EXISTING_JSON=$(jq 'del(.version)' "$FILEPATH")
    NEW_JSON=$(echo $DASHBOARD_JSON | jq 'del(.version)')

    # If the JSON (excluding the 'version' field) is the same, skip saving
    if [ "$EXISTING_JSON" == "$NEW_JSON" ]; then
      echo "Skipping $FILENAME - Only version has changed"
      continue
    fi
  fi

  # Save prettified JSON content to file
  echo $DASHBOARD_JSON | jq > "$FILEPATH"

  echo "Saved $FILENAME"
  COUNTER=$((COUNTER + 1)) # Increment the counter
done

echo "Total number of dashboards saved: $COUNTER"
