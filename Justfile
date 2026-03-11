# Run MCP integration tests in Docker.
# Requires: docker, just

set shell := ["bash", "-euo", "pipefail", "-c"]

image := "weather-forecast-mcp-tests"
container := "weather-forecast-mcp-tests"

# Build the test image
build:
  docker build -t {{image}} .

# Run the tests in a container (container deleted afterward)
test: build
  # Ensure we don't collide with a prior run
  docker rm -f {{container}} >/dev/null 2>&1 || true
  # --rm removes the container automatically on exit; rm -f is a safety net.
  # Pass through optional env toggles (if set on the host).
  docker run --name {{container}} --rm {{image}}
  docker rm -f {{container}} >/dev/null 2>&1 || true
