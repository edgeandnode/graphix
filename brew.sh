#!/bin/bash

brew install cask google-cloud-sdk
gcloud components install gke-gcloud-auth-plugin

curl -LO "https://dl.k8s.io/release/v1.24.1/bin/linux/amd64/kubectl"
chmod +x ./kubectl
mv ./kubectl /usr/local/bin/kubectl
