#!/usr/bin/env bash
set -e
# Get version from Cargo.toml
VERSION=$(grep version Cargo.toml | sed -n 's/.*version = "\([^"]*\)".*/\1/p;q')
echo $VERSION
# check whether we are on master and up to date and have no uncommitted changes
if [[ $(git rev-parse --abbrev-ref HEAD) != "master" ]]; then
    echo "Not on master branch. Aborting release."
    exit 1
fi

if [[ $(git status --porcelain) ]]; then
    echo "Uncommitted changes. Aborting release."
    exit 1
fi

git fetch origin
if [[ $(git rev-parse HEAD) != $(git rev-parse origin/master) ]]; then
    echo "Not up to date with origin/master. Aborting release."
    exit 1
fi

# Check whether Version is existing
if git rev-list $VERSION >/dev/null 2>&1; then
    echo "Version $VERSION already exists. Aborting release."
    exit 1
fi

git tag -a $VERSION -m "Release $VERSION"
git push origin $VERSION