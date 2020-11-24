#!/bin/bash

docker build -t my-geth:0.2 -f docker/geth/Dockerfile .
docker build -t ya-zksync-prover:0.2 -f docker/prover/Dockerfile .
