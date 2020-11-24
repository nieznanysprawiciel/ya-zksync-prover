#!/bin/bash

cd docker/geth
docker build -t my-geth:0.2 -f docker/geth/Dockerfile .

cd ../..
docker build -t ya-zksync-prover:0.2 -f docker/prover/Dockerfile .
