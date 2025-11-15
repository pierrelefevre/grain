# Deploying Grain

Grain is an OCI-compliant container registry that can be deployed in multiple ways.

## Using Docker

### Quick Start

```bash
# Pull the image
docker pull ghcr.io/pierrelefevre/grain:latest

# Create data directory and users.json
mkdir -p data
cat > data/users.json << 'EOF'
{
  "users": [
    {
      "username": "admin",
      "password": "changeme",
      "permissions": [
        {"repository": "*", "tag": "*", "actions": ["pull", "push", "delete"]}
      ]
    }
  ]
}
EOF

# Run the registry
docker run -d \
  --name grain \
  -p 8888:8888 \
  -v $(pwd)/data:/data \
  ghcr.io/pierrelefevre/grain:latest

# Check logs
docker logs -f grain
```

### Using Docker Compose

1. Download the `docker-compose.yml` file from the repository or create one:

```yaml
version: '3.8'

services:
  grain:
    image: ghcr.io/pierrelefevre/grain:latest
    container_name: grain-registry
    ports:
      - "8888:8888"
    volumes:
      - ./data:/data
      - grain-blobs:/data/blobs
      - grain-manifests:/data/manifests
      - grain-uploads:/data/uploads
    environment:
      - RUST_LOG=info
    restart: unless-stopped

volumes:
  grain-blobs:
  grain-manifests:
  grain-uploads:
```

2. Create the users.json file as shown above in a `data/` directory

3. Start the registry:

```bash
docker-compose up -d

# View logs
docker-compose logs -f grain

# Stop the registry
docker-compose down
```

### Environment Variables

- `RUST_LOG`: Logging level (debug, info, warn, error) - default: `info`

### Volumes

The `/data` directory contains all registry data:

- `/data/users.json`: User credentials and permissions
- `/data/blobs/`: Blob storage (content-addressable)
- `/data/manifests/`: Manifest storage
- `/data/uploads/`: Temporary upload sessions

### Ports

- `8888`: Registry HTTP port (default)

## Using Grain as a Docker Registry

Once running, you can use Docker CLI to interact with grain:

```bash
# Login to grain registry
docker login localhost:8888
# Enter username and password from users.json

# Tag an image
docker tag alpine:latest localhost:8888/myorg/alpine:latest

# Push the image
docker push localhost:8888/myorg/alpine:latest

# Pull the image
docker pull localhost:8888/myorg/alpine:latest

# List tags (using curl)
curl -u username:password http://localhost:8888/v2/myorg/alpine/tags/list
```

## Building from Source

### Prerequisites

- Rust 1.75 or later
- Cargo

### Build Steps

```bash
# Clone repository
git clone https://github.com/pierrelefevre/grain.git
cd grain

# Build release binary
cargo build --release

# Binary will be at target/release/grain
./target/release/grain --help
```

### Run from Source

```bash
# Create users.json
mkdir -p data
cat > data/users.json << 'EOF'
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
EOF

# Run the server
./target/release/grain --host 0.0.0.0:8888 --users-file ./data/users.json
```

## Production Deployment

### Behind Reverse Proxy (nginx)

For production use, run grain behind a reverse proxy with HTTPS:

```nginx
server {
    listen 443 ssl http2;
    server_name registry.example.com;

    ssl_certificate /path/to/cert.pem;
    ssl_certificate_key /path/to/key.pem;

    # Disable request size limits for large image pushes
    client_max_body_size 0;
    chunked_transfer_encoding on;

    location / {
        proxy_pass http://localhost:8888;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        proxy_set_header X-Forwarded-For $proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto $scheme;
        
        # Disable buffering for chunked uploads
        proxy_request_buffering off;
        proxy_buffering off;
    }
}
```

### Kubernetes Deployment

```yaml
apiVersion: v1
kind: PersistentVolumeClaim
metadata:
  name: grain-data
spec:
  accessModes:
    - ReadWriteOnce
  resources:
    requests:
      storage: 100Gi
---
apiVersion: v1
kind: ConfigMap
metadata:
  name: grain-users
data:
  users.json: |
    {
      "users": [
        {
          "username": "admin",
          "password": "changeme",
          "permissions": [
            {"repository": "*", "tag": "*", "actions": ["pull", "push", "delete"]}
          ]
        }
      ]
    }
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: grain
spec:
  replicas: 1
  selector:
    matchLabels:
      app: grain
  template:
    metadata:
      labels:
        app: grain
    spec:
      containers:
      - name: grain
        image: ghcr.io/pierrelefevre/grain:latest
        ports:
        - containerPort: 8888
          name: http
        volumeMounts:
        - name: data
          mountPath: /data
        - name: users
          mountPath: /data/users.json
          subPath: users.json
        env:
        - name: RUST_LOG
          value: info
        livenessProbe:
          httpGet:
            path: /v2/
            port: 8888
          initialDelaySeconds: 5
          periodSeconds: 30
        readinessProbe:
          httpGet:
            path: /v2/
            port: 8888
          initialDelaySeconds: 5
          periodSeconds: 10
        resources:
          requests:
            memory: "128Mi"
            cpu: "100m"
          limits:
            memory: "512Mi"
            cpu: "1000m"
      volumes:
      - name: data
        persistentVolumeClaim:
          claimName: grain-data
      - name: users
        configMap:
          name: grain-users
---
apiVersion: v1
kind: Service
metadata:
  name: grain
spec:
  selector:
    app: grain
  ports:
  - port: 8888
    targetPort: 8888
    name: http
  type: ClusterIP
---
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: grain
  annotations:
    cert-manager.io/cluster-issuer: letsencrypt-prod
spec:
  tls:
  - hosts:
    - registry.example.com
    secretName: grain-tls
  rules:
  - host: registry.example.com
    http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: grain
            port:
              number: 8888
```

## Administration

Grain includes a CLI tool `grainctl` for administration tasks:

```bash
# Add a user
grainctl add-user \
  --username newuser \
  --password secret \
  --repository "myorg/*" \
  --tag "*" \
  --actions pull,push

# Remove a user
grainctl remove-user --username newuser

# List users
grainctl list-users

# Add permissions to existing user
grainctl add-permission \
  --username existinguser \
  --repository "team/*" \
  --tag "v*" \
  --actions pull
```

Alternatively, use the HTTP admin API:

```bash
# Add user
curl -X POST http://localhost:8888/admin/users \
  -u admin:admin \
  -H "Content-Type: application/json" \
  -d '{
    "username": "newuser",
    "password": "secret",
    "permissions": [
      {"repository": "myorg/*", "tag": "*", "actions": ["pull", "push"]}
    ]
  }'

# List users
curl http://localhost:8888/admin/users -u admin:admin
```

## Security Considerations

1. **Change Default Passwords**: Always change default passwords in production
2. **Use HTTPS**: Never expose grain directly to the internet without TLS
3. **Granular Permissions**: Use repository and tag-level permissions to limit access
4. **Network Isolation**: Deploy in private networks when possible
5. **Regular Updates**: Keep grain updated to the latest version
6. **Backup**: Regularly backup the `/data` directory

## Troubleshooting

### Cannot push large images

Ensure your reverse proxy is configured to allow large requests:
- nginx: `client_max_body_size 0;`
- Set `proxy_request_buffering off;`

### Permission denied errors

Check that:
1. User exists in `users.json`
2. User has appropriate permissions for the repository/tag
3. Authentication credentials are correct

### Container fails to start

Check logs:
```bash
docker logs grain
```

Common issues:
- Missing or invalid `users.json`
- Port 8888 already in use
- Volume mount permissions

### Health check failing

Verify the registry is responding:
```bash
curl http://localhost:8888/v2/
```

Should return: `{"status":"ok"}`

## Performance Tuning

### Filesystem Backend

Grain uses a simple filesystem backend. For better performance:

1. Use fast storage (SSD/NVMe)
2. Consider dedicated volumes for blobs and manifests
3. Use XFS or ext4 filesystems
4. Enable filesystem caching

### Resource Limits

Recommended minimum resources:
- CPU: 1 core
- Memory: 512MB
- Storage: 100GB (depends on registry size)

For high-traffic deployments:
- CPU: 4+ cores
- Memory: 2GB+
- Consider using multiple instances behind a load balancer

## Monitoring

Monitor these metrics:

1. Disk usage (blobs/manifests directories)
2. Memory usage
3. Request latency
4. Error rates
5. Authentication failures

Example Prometheus metrics endpoint (future feature):
```
http://localhost:8888/metrics
```

## Backup and Recovery

### Backup

```bash
# Stop the registry
docker-compose down

# Backup data directory
tar -czf grain-backup-$(date +%Y%m%d).tar.gz data/

# Restart registry
docker-compose up -d
```

### Recovery

```bash
# Stop registry
docker-compose down

# Restore from backup
tar -xzf grain-backup-20240115.tar.gz

# Restart registry
docker-compose up -d
```

## Migration

To migrate from another registry:

1. Export images from old registry
2. Tag with new registry URL
3. Push to grain
4. Update deployment manifests

```bash
# Example migration
docker pull old-registry.com/myorg/app:v1
docker tag old-registry.com/myorg/app:v1 grain.example.com/myorg/app:v1
docker push grain.example.com/myorg/app:v1
```

## Support

For issues and questions:
- GitHub Issues: https://github.com/pierrelefevre/grain/issues
- Documentation: https://github.com/pierrelefevre/grain
