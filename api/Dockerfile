# Сборка отдельно от рантайма: в финальный образ едет один бинарник,
# без cargo, без исходников, без тулчейна.
FROM rust:1.97-slim-bookworm AS build
WORKDIR /app
RUN apt-get update && apt-get install -y --no-install-recommends pkg-config && rm -rf /var/lib/apt/lists/*

# Зависимости отдельным слоем: правка src не пересобирает крейты заново.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main(){}' > src/main.rs && cargo build --release && rm -rf src

COPY migrations ./migrations
COPY src ./src
RUN touch src/main.rs && cargo build --release

# distroless: только ca-certificates и libc, ни shell, ни пакетного менеджера
FROM gcr.io/distroless/cc-debian12:nonroot
COPY --from=build /app/target/release/leser-api /usr/local/bin/leser-api
EXPOSE 4444
CMD ["/usr/local/bin/leser-api"]
