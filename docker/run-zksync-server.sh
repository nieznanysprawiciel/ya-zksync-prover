#!/bin/bash

mkdir -p volumes
mkdir -p volumes/geth
mkdir -p volumes/postgres
mkdir -p volumes/tesseracts

USER_ID="$(id -u)"
GROUP_ID="$(id -g)"
export CURRENT_UID="${USER_ID}:${GROUP_ID}"

echo "Runnig as user with ids: ${CURRENT_UID}"
docker-compose up -d

