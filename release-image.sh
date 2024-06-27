#!/usr/bin/env bash
# Get version from Cargo.toml
VERSION=$(grep version Cargo.toml | sed -n 's/.*version = "\([^"]*\)".*/\1/p;q')
echo $VERSION
docker build -t gfreezy/mailhook:$VERSION -t gfreezy/mailhook:latest .
docker push gfreezy/mailhook:$VERSION
docker push gfreezy/mailhook:latest