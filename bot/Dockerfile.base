# Stage 1: Build the application
FROM docker.io/rust:latest as builder

RUN apt-get update && apt-get install -y cmake && apt install -y libc6

# Set the working directory
WORKDIR /

# Copy the Cargo.toml and Cargo.lock files
COPY Cargo.toml Cargo.lock ./

# Create a new empty shell project to cache dependencies
#RUN mkdir src && echo "fn main() {}" > src/main.rs

COPY src src
COPY bin bin 
COPY IDL IDL
#COPY proto proto


# Build the dependencies
RUN cargo build --release
RUN rm -r src

# Stage 2: Create the final image
FROM docker.io/rust:latest
RUN apt-get update && apt install -y libc6
# Set the working directory
WORKDIR /usr/src/myapp

# Copy the compiled binary from the builder
COPY --from=builder /target/release/ .

# Specify the command to run the application
CMD ["./process"]