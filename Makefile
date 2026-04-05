.PHONY: build test clean up down logs migrate

# Build all workspace members
build:
	cargo build --workspace

# Build in release mode
build-release:
	cargo build --workspace --release

# Run all tests
test:
	cargo test --workspace

# Run tests for a specific crate
test-%:
	cargo test -p saas-$*

# Clean build artifacts
clean:
	cargo clean
	rm -rf data/

# Docker operations
up:
	docker compose --env-file .env -f deployments/docker-compose.yml up -d

down:
	docker compose --env-file .env -f deployments/docker-compose.yml down

logs:
	docker compose --env-file .env -f deployments/docker-compose.yml logs -f

# Run migrations for all services
migrate:
	@echo "Migrations run automatically on service startup"

# Start NATS locally
nats:
	docker run -d --name nats -p 4222:4222 -p 8222:8222 nats:2-alpine --jetstream

# Stop local NATS
nats-stop:
	docker stop nats && docker rm nats

# Run a single service locally (usage: make run-iam, make run-employee, etc.)
run-%:
	cargo run -p saas-$*

# Check compilation without building
check:
	cargo check --workspace

# Format code
fmt:
	cargo fmt --all

# Lint
lint:
	cargo clippy --workspace -- -D warnings
