#!/bin/bash
# Run from ./workdir directory in ya-zksync-prover repo
# or other directory, where you run prover Requestor.

docker run --rm -v "$(pwd)"/blocks:/blocks -v "$(pwd)"/proofs:/proofs ya-zksync-prover:0.1
