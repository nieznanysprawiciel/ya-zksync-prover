#!/bin/bash
# Yagna Requestor (ya-zksync-node binary) places all downloaded artifacts
# in the same directory structure as the will appear on Provider, so you can run prover
# docker in working directory

mkdir -p workdir

cd workdir/
docker run --rm -v "$(pwd)"/blocks:/blocks -v "$(pwd)"/proofs:/proofs ya-zksync-prover:0.1
