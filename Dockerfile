FROM rust:1.73 as build

# create a new empty shell project
RUN cargo new --bin cs2-discord-scrimbot
WORKDIR /cs2-discord-scrimbot

COPY . .

RUN cargo build --release

# our final base
FROM rust:1.73-slim-buster

# copy the build artifact from the build stage
COPY --from=build /cs2-discord-scrimbot/target/release/cs2-discord-scrimbot .

# set the startup command to run your binary
CMD ["./cs2-discord-scrimbot"]
