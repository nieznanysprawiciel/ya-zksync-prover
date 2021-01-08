#!/bin/bash

set -e
source benchmark/.env

TIMEFORMAT="%E"

for i in {1..10}
do
   time ./benchmark/transfer.sh ${TRANSFER_TO}
done


