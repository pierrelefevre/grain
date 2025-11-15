# pierrelefevre/grain
Rust implementation of OCI Distribution Spec with granular access control

## Installation

### Using Docker

```bash
docker pull ghcr.io/pierrelefevre/grain:latest
docker run -p 8888:8888 -v $(pwd)/data:/data ghcr.io/pierrelefevre/grain:latest
```

See [docs/deployment.md](docs/deployment.md) for complete deployment guide.

### Using Docker Compose

```bash
# Download docker-compose.yml from repository
docker-compose up -d
```

### From Source

```bash
git clone https://github.com/pierrelefevre/grain.git
cd grain
cargo build --release
./target/release/grain --host 0.0.0.0:8888 --users-file ./data/users.json
```

## Quick Start

1. Create a `data/users.json` file:
```json
{
  "users": [
    {
      "username": "admin",
      "password": "admin",
      "permissions": [
        {"repository": "*", "tag": "*", "actions": ["pull", "push", "delete"]}
      ]
    }
  ]
}
```

2. Run the registry:
```bash
docker run -p 8888:8888 -v $(pwd)/data:/data ghcr.io/pierrelefevre/grain:latest
```

3. Use with Docker:
```bash
docker login localhost:8888
docker tag alpine:latest localhost:8888/myorg/alpine:latest
docker push localhost:8888/myorg/alpine:latest
```

## Goals
- Implement the OCI Distribution Spec in Rust
- Use local filesystem for storage
- Use basic auth scheme
- Provide granular access control per tag
- Administration API to manage permissions
- Publish the registry as a container image on GHCR
- CLI tool for administration tasks

## Admin API
- Add/remove users
- Set pull permission for user on tag
- Interactive API documentation available at `/swagger-ui/` when server is running
- OpenAPI schema available at `/api-docs/openapi.json`

### Admin API Endpoints

**Authentication**: All admin endpoints require HTTP Basic Auth with admin privileges (user must have wildcard delete permission on `*/*`).

**GET /admin/users** - List all users with their permissions

**POST /admin/users** - Create a new user
```json
{
  "username": "string",
  "password": "string",
  "permissions": [
    {
      "repository": "string",
      "tag": "string",
      "actions": ["pull", "push", "delete"]
    }
  ]
}
```

**DELETE /admin/users/{username}** - Delete a user (cannot delete yourself)

**POST /admin/users/{username}/permissions** - Add permission to a user
```json
{
  "repository": "string",
  "tag": "string",
  "actions": ["pull", "push", "delete"]
}
```

## CLI Administration Tool

A separate `grainctl` binary is provided for easy administration via command line.

### Installation
```bash
cargo build --release
# Binary will be at target/release/grainctl
```

### Configuration
Set environment variables to avoid repeating credentials:
```bash
export GRAIN_URL=http://localhost:8888
export GRAIN_ADMIN_USER=admin
export GRAIN_ADMIN_PASSWORD=admin
```

Or use command-line flags for each command.

### Commands

**List all users:**
```bash
grainctl user list
# or with explicit credentials:
grainctl user list --url http://localhost:8888 --username admin --password admin
```

**Create a new user:**
```bash
grainctl user create alice --pass alicepass
```

**Delete a user:**
```bash
grainctl user delete alice
```

**Add permission to a user:**
```bash
grainctl user add-permission alice \
  --repository "myorg/myapp" \
  --tag "dev" \
  --actions "pull,push"
```

Use wildcards for broader permissions:
```bash
grainctl user add-permission alice \
  --repository "myorg/*" \
  --tag "*" \
  --actions "pull"
```

## Spec
[OCI Distribution Spec v1.1.1](spec.md)