ARG COPY_ADDRESSES=copy

FROM ubuntu:jammy-20231211.1 as build_copy
ONBUILD COPY ./azero_addresses.json /usr/local/contracts/azero_addresses.json
ONBUILD COPY ./eth_addresses.json /usr/local/contracts/eth_addresses.json

FROM ubuntu:jammy-20231211.1 as build_no_copy
ONBUILD RUN echo "Contract addresses are not copied"


FROM build_${COPY_ADDRESSES}
RUN apt update && \
    apt install ca-certificates -y && \
    apt install jq -y && \
    apt clean && \
    rm -rf /var/lib/apt/lists/*

RUN update-ca-certificates

COPY target/release/relayer /usr/local/bin/most-relayer
RUN chmod +x /usr/local/bin/most-relayer

COPY scripts/entrypoint.sh /usr/local/bin
RUN chmod +x /usr/local/bin/entrypoint.sh

COPY ./most.json /usr/local/most.json
COPY ./advisory.json /usr/local/advisory.json
COPY ./token.json /usr/local/token.json
COPY ./router_contract.json /usr/local/router.json

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
