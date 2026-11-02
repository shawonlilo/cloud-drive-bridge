# cdb as a container: script bucket checks/transfers on any server.
FROM rust:1-alpine AS build
WORKDIR /src
COPY Cargo.toml ./
COPY core ./core
COPY cli ./cli
RUN cargo build --release --bin cdb

FROM alpine:3.20
RUN adduser -D cdb
COPY --from=build /src/target/release/cdb /usr/local/bin/cdb
USER cdb
ENTRYPOINT ["cdb"]
CMD ["--help"]
