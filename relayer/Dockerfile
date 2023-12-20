FROM ubuntu:jammy-20231211.1

#RUN apt update && \
#    apt install wget -y && \
#    apt clean

RUN apt update && \
    apt install ca-certificates -y && \
    apt install jq -y && \
    apt clean && \
    rm -rf /var/lib/apt/lists/*

RUN update-ca-certificates

#RUN wget http://archive.ubuntu.com/ubuntu/pool/main/o/openssl/libssl1.1_1.1.1f-1ubuntu2_amd64.deb
#RUN dpkg -i libssl1.1_1.1.1f-1ubuntu2_amd64.deb

COPY target/release/relayer /usr/local/bin/most-relayer
RUN chmod +x /usr/local/bin/most-relayer

COPY scripts/entrypoint.sh /usr/local/bin
RUN chmod +x /usr/local/bin/entrypoint.sh

COPY ./azero_addresses.json /usr/local/azero_addresses.json
COPY ./eth_addresses.json /usr/local/eth_addresses.json

ENTRYPOINT ["/usr/local/bin/entrypoint.sh"]
