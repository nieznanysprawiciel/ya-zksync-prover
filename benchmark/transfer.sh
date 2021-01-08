#!/bin/bash
set -e

TRANSFER_TO=$1

HASH=$(./zcli transfer 0.001 ETH ${TRANSFER_TO} | jq '.transaction.hash' | tr -d \")
./zcli await -t 400000 verify ${HASH} > /dev/null 2>&1
