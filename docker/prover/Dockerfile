FROM rust:1.48 as builder
WORKDIR /usr/src/zksync
COPY ./yagna-prover .
RUN cargo build --release

FROM matterlabs/prover:47bb16f

ENV SUPPORTED_BLOCK_CHUNKS_SIZES 6,30,74,150,320,630
ENV SUPPORTED_BLOCK_CHUNKS_SIZES_SETUP_POWERS 21,22,23,24,25,26
ENV BLOCK_CHUNK_SIZES 6,30
ENV KEY_DIR "keys/plonk-975ae851"
ENV ACCOUNT_TREE_DEPTH 32
ENV BALANCE_TREE_DEPTH 11
ENV PROVER_SERVER_URL "http://127.0.0.1:8088"
ENV REQ_SERVER_TIMEOUT 10
#ENV RUST_BACKTRACE $RUST_BACKTRACE
ENV RUST_LOG "zksync_api=debug,zksync_core=debug,zksync_eth_sender=debug,zksync_witness_generator=debug,zksync_server=debug,zksync_prover=debug,dummy_prover=info,key_generator=info,zksync_data_restore=info,zksync_eth_client=info,zksync_storage=info,zksync_state=info,zksync_types=info,exodus_test=info,loadtest=info,kube=debug,dev_ticker=info,block_sizes_test=info"
ENV PROVER_DOWNLOAD_SETUP "false"
ENV PROVER_SETUP_NETWORK_DIR "-"
ENV PROVER_PREPARE_DATA_INTERVAL 500
ENV PROVER_HEARTBEAT_INTERVAL 1000
ENV PROVER_CYCLE_WAIT 500
ENV PROVER_GONE_TIMEOUT 60000
ENV DOCKER_DUMMY_PROVER "false"

COPY --from=builder /usr/src/zksync/target/release/yagna-prover /bin/
COPY docker/prover/download-keys.sh /bin/
COPY docker/prover/ya-entry.sh /bin/
RUN download-keys.sh

VOLUME /blocks /proofs


CMD []
WORKDIR /
ENTRYPOINT ["ya-entry.sh"]
