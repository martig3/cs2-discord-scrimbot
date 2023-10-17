FROM rust:alpine AS build

RUN apk add --no-cache build-base libressl-dev && mkdir -p /app
COPY . /app
WORKDIR /app
RUN cargo build --release && strip target/release/cs2-discord-scrimbot

FROM scratch
COPY --from=build /app/target/release/cs2-discord-scrimbot .
CMD [ "/cs2-discord-scrimbot" ]
