# Yagna zksync prover

Runs zksync prover on yagna providers.

## Instruction

### Running ya-zksync-prover with local zksync server

#### Preparing environment

- You need zksync dev environment running locally. WARNING: It's not easy!!
   - Clone  zksync repo `git clone https://github.com/matter-labs/zksync.git` 
   - Install dependencies and setup environment following this instruction: https://github.com/matter-labs/zksync/blob/master/docs/setup-dev.md
   - Launch zksync locally using instruction: https://github.com/matter-labs/zksync/blob/master/docs/launch.md This will initialize databases
     and download keys.
   - You will need to be able to send transaction, but it's hard to get ETH on local Geth.
     One way to workaround this, is to replace Geth docker image in docker-compose.yml in zksync repo
     with image, that have initialized accounts, that you want. Check `docker/geth` directory, how I did it.
 
- Install `Yagna` Provider (TODO: add links after next release). 
- `Yagna` Requestor doesn't need separate installation, but create app keys
  following instruction: https://handbook.golem.network/requestor-tutorials/flash-tutorial-of-requestor-development
            
#### Running `zksync` prover on `Yagna` 

If you have environment prepared
- Launch zksync environment `zk up`. This starts docker-compose with local Geth and
some other stuff necessary for zksync server to work. 
- Launch zksync server `zk server`.
- Launch `Yagna` Provider or use public Providers network.
    - If you decide to setup only local provider, remember to set `subnet` parameter.
- Launch `Yagna` Requestor
    - Set `YAGNA_APPKEY` environment variable in your `.env` or export as shell variable.
    (Use `.env-template` as example)
    - `yagna service run`
- Run `ya-zksync-prover`. In repo directory type: 
    ```
    mkdir -p workdir
    cp .env-template workdir/.env
    cd workdir
    cargo run
    ``` 
- Add transactions to zksync server. To do this you can use `zcli`. For example:
    ```
    ./zcli deposit 5 ETH 0x0532c4b81d77cbc75f05bb41cedeb1bfb31d6d77
    ./zcli transfer 0.01 ETH 0x91b91be45d70896ed8376384bff01367660f4ae9
    ```
- `ya-zksync-prover` should start proving blocks now.
 
### Building dockers manually

To build docker images run:
```
./docker/build-dockers.sh
```
This will build ya-zksync-prover image, that can be converted to vm image
and my-geth, that is usefull for running zksync server with initialized accounts.

### Debugging docker image

Yagna Requestor (ya-zksync-node binary) places all downloaded artifacts in the same
directory structure as the will appear on Provider.
You can run `ya-zksync-prover` image locally to check if it works properly.

You need blocks and job information to be able to compute proof. You can run Requestor agent
that will download all data from zksync server and will place it in your workind directory.
