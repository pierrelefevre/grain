# AGENTS.md

## Project Overview

**grain** is a Rust implementation of the [OCI Distribution Specification v1.1.1](https://github.com/opencontainers/distribution-spec) - a container registry server with granular access control.

### What It Is
A container registry that stores and distributes OCI-compliant container images and artifacts using a filesystem backend. It provides HTTP APIs for pulling/pushing container images, manifests, and blobs, with basic authentication.

### What It's Trying To Be
A production-ready, lightweight OCI registry server featuring:
- Full OCI Distribution Spec compliance
- Filesystem-based storage (blobs and manifests stored in `./tmp/`)
- HTTP Basic Authentication
- Granular tag-level access control
- Administration API for user and permission management
- Publishable as a container image on GHCR
- CLI tools for administration

### Current State
**Full implementation** - all core OCI endpoints and administration features are complete:
- ✅ Basic auth (end-1: `/v2/`)
- ✅ Blob uploads (end-4a/4b: `POST /v2/<name>/blobs/uploads/`)
- ✅ Manifest uploads (end-7: `PUT /v2/<name>/manifests/<reference>`)
- ✅ Blob/manifest retrieval (end-2, end-3)
- ✅ Tag listing (end-8a/8b)
- ✅ Deletion endpoints (end-9, end-10)
- ✅ Chunked upload operations (end-5, end-6)
- ✅ Cross-repo blob mounting (end-11)
- ✅ Granular tag-level permissions
- ✅ Administration API
- ✅ CLI administration tool (`grainctl`)

## Architecture

### Tech Stack
- **Language**: Rust (edition 2021)
- **Web Framework**: Axum 0.8.3
- **Runtime**: Tokio (async)
- **Auth**: HTTP Basic Auth (base64-encoded credentials)
- **Storage**: Local filesystem (`./tmp/blobs/`, `./tmp/manifests/`)
- **Logging**: env_logger + log crate

### Module Structure
```
src/
├── main.rs       - Router setup, endpoint registration, server startup
├── args.rs       - CLI argument parsing (host, users_file)
├── state.rs      - Shared app state (server status, users, config)
├── auth.rs       - HTTP Basic Auth parsing and validation
├── response.rs   - HTTP response helpers (unauthorized, not_found, forbidden, etc.)
├── storage.rs    - Filesystem I/O for blobs/manifests
├── blobs.rs      - Blob endpoints (GET, HEAD, POST, PATCH, PUT, DELETE)
├── manifests.rs  - Manifest endpoints (GET, HEAD, PUT, DELETE)
├── tags.rs       - Tag listing endpoints
├── admin.rs      - Administration API (user/permission management)
├── permissions.rs - Permission checking logic
├── meta.rs       - Index and catch-all routes
├── utils.rs      - Build version helper
└── bin/
    └── grainctl.rs - CLI tool for administration (separate binary)
```

### Data Flow
1. **Request** → Axum router matches endpoint
2. **Authentication** → `auth::get` validates Basic Auth header against `users.json`
3. **Handler** → Module-specific handler (blobs/manifests/tags)
4. **Storage** → `storage.rs` writes/reads from filesystem with digest validation
5. **Response** → Standardized HTTP response with appropriate headers

### Storage Layout
```
tmp/
├── users.json           - User credentials (username/password pairs)
├── blobs/
│   └── {org}/
│       └── {repo}/
│           └── {sha256}  - Content-addressable blob files
└── manifests/
    └── {org}/
        └── {repo}/
            └── {reference}  - Manifest files (tags or digests)
```

### State Management
- **App State**: `Arc<state::App>` shared across handlers
  - `Mutex<ServerStatus>` - startup state tracking
  - `Mutex<HashSet<User>>` - in-memory user database loaded from `users.json`
  - `Args` - CLI configuration (host, users_file path)

## Development Guidelines

### Pre-Commit Requirements
Every solution MUST pass all three checks before being presented:

```bash
cargo build          # Must compile without errors
cargo clippy         # Treat warnings as errors - fix ALL clippy warnings
cargo fmt            # Apply standard Rust formatting
```

### Code Quality Standards
- **No dead code** - Remove all unused functions, imports, variables
- **No warnings** - `cargo clippy` output must be clean
- **No useless comments** - Self-documenting code preferred; comments only for complex logic
- **No backwards compatibility** - Break things if needed for cleaner design
- **Idiomatic Rust** - Use proper error handling, pattern matching, ownership patterns

### Testing Workflow
```bash
# Build and check
cargo build
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --check

# Run server
cargo run -- --host 0.0.0.0:8888 --users-file ./tmp/users.json

# Test endpoints (examples)
curl -u test:test http://localhost:8888/v2/
curl -u test:test -X POST http://localhost:8888/v2/myorg/myrepo/blobs/uploads/
```

### Key Implementation Patterns

#### Authentication
```rust
// All protected endpoints should validate auth via headers
let auth_header = headers.get("authorization")?;
// Parse "Basic base64(username:password)"
// Match against loaded users from state
```

#### Digest Validation
```rust
// Blobs MUST validate SHA256 digest matches content
let body_digest = sha256::digest(bytes.as_ref());
let req_digest = digest_string.strip_prefix("sha256:").unwrap_or(digest_string);
if req_digest != body_digest { return false; }
```

#### Error Responses
```rust
// Use response.rs helpers for consistency
response::unauthorized(&host)    // 401 + WWW-Authenticate header
response::not_found()            // 404
response::not_implemented()      // 501
response::ok()                   // 200
```

#### Path Sanitization
```rust
// Always sanitize org/repo names before filesystem operations
fn sanitize_string(input: &str) -> String {
    // Allow only alphanumeric, '.', '_', '-', '/'
}
```

## OCI Distribution Spec Endpoints

Reference table from `spec.md` - implement in order of priority:

| ID     | Method         | Endpoint                                                      | Status    | Priority |
|--------|----------------|---------------------------------------------------------------|-----------|----------|
| end-1  | GET            | `/v2/`                                                        | ✅ Done    | 1        |
| end-2  | GET/HEAD       | `/v2/<name>/blobs/<digest>`                                   | ✅ Done    | 2        |
| end-3  | GET/HEAD       | `/v2/<name>/manifests/<reference>`                            | ✅ Done    | 3        |
| end-4a | POST           | `/v2/<name>/blobs/uploads/`                                   | ✅ Done    | 4        |
| end-4b | POST           | `/v2/<name>/blobs/uploads/?digest=<digest>`                   | ✅ Done    | 5        |
| end-5  | PATCH          | `/v2/<name>/blobs/uploads/<reference>`                        | ✅ Done    | 7        |
| end-6  | PUT            | `/v2/<name>/blobs/uploads/<reference>?digest=<digest>`        | ✅ Done    | 8        |
| end-7  | PUT            | `/v2/<name>/manifests/<reference>`                            | ✅ Done    | 6        |
| end-8a | GET            | `/v2/<name>/tags/list`                                        | ✅ Done    | 9        |
| end-8b | GET            | `/v2/<name>/tags/list?n=<integer>&last=<integer>`             | ✅ Done    | 10       |
| end-9  | DELETE         | `/v2/<name>/manifests/<reference>`                            | ✅ Done    | 11       |
| end-10 | DELETE         | `/v2/<name>/blobs/<digest>`                                   | ✅ Done    | 12       |
| end-11 | POST           | `/v2/<name>/blobs/uploads/?mount=<digest>&from=<other_name>`  | ✅ Done    | 13       |

### Implementation Notes

#### end-2: GET/HEAD Blob by Digest
Must return blob content from `./tmp/blobs/{org}/{repo}/{digest}`:
- HEAD: Return 200 + Content-Length header if exists, 404 otherwise
- GET: Stream file contents with `Content-Type: application/octet-stream`
- Add `Docker-Content-Digest: sha256:{digest}` header

#### end-3: GET/HEAD Manifest by Reference
Must return manifest JSON from `./tmp/manifests/{org}/{repo}/{reference}`:
- HEAD: Return 200 + Content-Length + Content-Type if exists
- GET: Return JSON manifest with proper Content-Type (e.g., `application/vnd.oci.image.manifest.v1+json`)
- Add `Docker-Content-Digest` header with computed SHA256

#### end-5/end-6: Chunked Upload
Implement resumable blob uploads:
- PATCH: Append chunk to temporary upload file, return 202 + Range header
- PUT with digest: Finalize upload, validate digest, move to final location
- Track upload sessions (UUIDs) in memory or filesystem

#### end-8a/end-8b: Tag Listing
Scan `./tmp/manifests/{org}/{repo}/` directory:
- Return JSON: `{"name": "{org}/{repo}", "tags": ["tag1", "tag2", ...]}`
- Support pagination with `n` (limit) and `last` (cursor) query params

#### end-9/end-10: Deletion
- end-9: Delete manifest file, return 202
- end-10: Delete blob file if no manifests reference it (garbage collection consideration)

## Missing Features (TODO)

### High Priority
1. **Error Handling** - Proper OCI error response format with error codes (see spec.md)

### Medium Priority
1. **Validation** - Manifest schema validation (OCI image manifest, image index)
2. **Garbage Collection** - Clean up unreferenced blobs
3. **Metrics/Health** - Prometheus metrics, health check endpoint

### Low Priority
1. **TLS Support** - HTTPS configuration
2. **Docker Image** - Dockerfile for GHCR publishing
3. **Referrers API** - Support for artifact references (spec extension)

## Common Tasks

### Add a New Endpoint
1. Define handler in appropriate module (blobs/manifests/tags)
2. Register route in `main.rs` router with correct HTTP method
3. Add auth check if needed (pass `State<Arc<state::App>>`)
4. Implement storage logic in `storage.rs` if needed
5. Return appropriate response using `response.rs` helpers
6. Test with curl/docker CLI
7. Run `cargo clippy` and fix all warnings

### Modify Storage Logic
1. Update functions in `storage.rs`
2. Maintain digest validation for blobs
3. Ensure directory creation with `create_dir_all`
4. Add comprehensive error logging
5. Test with actual blob/manifest uploads

### Add User Management
1. Create new `admin.rs` module for admin endpoints
2. Add routes like `POST /admin/users`, `DELETE /admin/users/{username}`
3. Modify `state.rs` to support runtime user updates (write back to `users.json`)
4. Add admin-only auth middleware (separate from basic user auth)

### Implement Permissions
1. Extend `User` struct in `state.rs` with permissions field
2. Add `permissions: HashMap<String, Vec<String>>` (tag → allowed operations)
3. Create middleware to check permissions before blob/manifest operations
4. Update `users.json` schema to include permissions

## Debugging Tips

### Enable Verbose Logging
```bash
RUST_LOG=debug cargo run
```

### Check Stored Files
```bash
# List all blobs
find ./tmp/blobs -type f

# List all manifests
find ./tmp/manifests -type f

# Verify blob digest
shasum -a 256 ./tmp/blobs/{org}/{repo}/{digest}
```

### Test with Docker Client
```bash
# Login
docker login localhost:8888 -u test -p test

# Push image (requires full OCI spec compliance)
docker tag alpine:latest localhost:8888/myorg/myrepo:latest
docker push localhost:8888/myorg/myrepo:latest

# Pull image
docker pull localhost:8888/myorg/myrepo:latest
```

### Common Issues
- **401 on /v2/**: Check Basic Auth header format: `Basic base64(username:password)`
- **Digest mismatch**: Ensure SHA256 calculation matches OCI spec (sha256 of raw bytes)
- **File not found**: Verify path sanitization doesn't break org/repo with special chars
- **Clippy warnings**: Fix immediately - they indicate potential bugs or non-idiomatic code

## Resources

- [OCI Distribution Spec v1.1.1](spec.md) - Full specification reference
- [OCI Image Spec](https://github.com/opencontainers/image-spec) - Manifest/config formats
- [Axum Documentation](https://docs.rs/axum) - Web framework reference
- [Docker Registry API v2](https://docs.docker.com/registry/spec/api/) - Historical context

## Development Checklist

Before committing any change:
- [ ] `cargo build` succeeds
- [ ] `cargo clippy` reports zero warnings
- [ ] `cargo fmt` applied
- [ ] No dead code remains
- [ ] All imports used
- [ ] Logging added for new operations
- [ ] Error cases handled properly
- [ ] Manual testing completed (curl or docker client)
