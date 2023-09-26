FROM ubuntu:jammy-20220531

RUN apt update && \
        apt install curl -y && \
        apt clean && \
        rm -rf /var/lib/apt/lists/*

EXPOSE 80

WORKDIR relayer

COPY target/release/relayer /usr/local/bin
RUN chmod +x /usr/local/bin/relayer

COPY docker/entrypoint.sh /entrypoint.sh
RUN chmod +x /entrypoint.sh

ENTRYPOINT ["./entrypoint.sh"]
