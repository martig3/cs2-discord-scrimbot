FROM rust:1.69 as build

# create a new empty shell project
RUN cargo new --bin csgo-discord-scrimbot
WORKDIR /csgo-discord-scrimbot

COPY . .

RUN cargo build --release

# our final base
FROM rust:1.69-slim-buster

# copy the build artifact from the build stage
COPY --from=build /csgo-discord-scrimbot/target/release/csgo-discord-scrimbot .

# set the startup command to run your binary
CMD ["./csgo-discord-scrimbot"]
