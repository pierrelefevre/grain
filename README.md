# pierrelefevre/grain
Rust implementation of OCI Distribution Spec with granular access control

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

## Spec
[OCI Distribution Spec v1.1.1](spec.md)