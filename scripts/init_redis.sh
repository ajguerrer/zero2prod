#!/usr/bin/env bash
set -exo pipefail

# Allow to skip Docker if a dockerized Redis database is already running
if [[ -z "${SKIP_DOCKER}" ]]; then
    # if a Redis container is running, print instructions to kill it and exit
    RUNNING_CONTAINER=$(docker ps --filter 'name=redis' --format '{{.ID}}')
    if [[ -n $RUNNING_CONTAINER ]]; then
      echo >&2 "there is a redis container already running, kill it with"
      echo >&2 "    docker kill ${RUNNING_CONTAINER}"
      exit 1
    fi

    # Launch Redis using Docker
    docker run \
        -p "6379:6379" \
        -d \
        --name "zero2prod-redis" \
        redis:7
fi

>&2 echo "Redis is ready to go!"