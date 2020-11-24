# Yagna zksync prover

Runs zksync prover on yagna providers.

## Instruction

### Building and preparing environment

To build docker images run:
```
./docker/build-dockers.sh
```
This will build ya-zksync-prover image, that can be converted to vm image
and my-geth, that is usefull for running zksync server with initialized accounts.

### Running ya-zksync-prover locally

Yagna Requestor (ya-zksync-node binary) places all downloaded artifacts in the same
directory structure as the will appear on Provider.
You can run `ya-zksync-prover` image locally to check if it works properly.

You need blocks and job information to be able to compute proof. You can run Requestor agent
that will download all data from zksync server and will place it in your workind directory.
