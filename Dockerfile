FROM rust:1.60 as build

# create a new empty shell project
RUN USER=root cargo new --bin csgo-discord-scrimbot
WORKDIR /csgo-discord-scrimbot

COPY . .

RUN cargo build --release

# our final base
FROM rust:1.60-slim-buster

# copy the build artifact from the build stage
COPY --from=build /csgo-discord-scrimbot/target/release/csgo-discord-scrimbot .

# set the startup command to run your binary
CMD ["./csgo-discord-scrimbot"]
