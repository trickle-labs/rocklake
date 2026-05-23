# Kubernetes Deployment

Kubernetes is the recommended deployment platform for production SlateDuck instances. This page covers the deployment manifests, configuration patterns, and operational practices for running SlateDuck reliably on Kubernetes.

## Architecture

SlateDuck runs as a Deployment (not a StatefulSet) because it has no local state — all data is in object storage. This simplifies rolling updates, scaling, and failure recovery.

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: slateduck
  labels:
    app: slateduck
spec:
  replicas: 1
  selector:
    matchLabels:
      app: slateduck
  template:
    metadata:
      labels:
        app: slateduck
    spec:
      containers:
        - name: slateduck
          image: ghcr.io/slateduck/slateduck:0.8.0
          ports:
            - containerPort: 5432
              name: pgwire
          args:
            - "--storage"
            - "s3://$(CATALOG_BUCKET)/catalog/"
            - "--bind"
            - "0.0.0.0:5432"
          env:
            - name: CATALOG_BUCKET
              valueFrom:
                configMapKeyRef:
                  name: slateduck-config
                  key: bucket
            - name: AWS_REGION
              value: us-east-1
            - name: RUST_LOG
              value: info
          resources:
            requests:
              memory: "128Mi"
              cpu: "100m"
            limits:
              memory: "512Mi"
              cpu: "1000m"
          livenessProbe:
            tcpSocket:
              port: pgwire
            initialDelaySeconds: 5
            periodSeconds: 10
          readinessProbe:
            tcpSocket:
              port: pgwire
            initialDelaySeconds: 10
            periodSeconds: 5
      serviceAccountName: slateduck
```

## Service

Expose SlateDuck to other pods in the cluster:

```yaml
apiVersion: v1
kind: Service
metadata:
  name: slateduck
spec:
  selector:
    app: slateduck
  ports:
    - port: 5432
      targetPort: pgwire
      protocol: TCP
  type: ClusterIP
```

DuckDB pods connect via: `host=slateduck.default.svc.cluster.local;port=5432`

## IAM Authentication (AWS)

Use IAM Roles for Service Accounts (IRSA) to provide S3 access without static credentials:

```yaml
apiVersion: v1
kind: ServiceAccount
metadata:
  name: slateduck
  annotations:
    eks.amazonaws.com/role-arn: arn:aws:iam::123456789:role/slateduck-role
```

The IAM role needs permissions: `s3:GetObject`, `s3:PutObject`, `s3:DeleteObject`, `s3:ListBucket` on the catalog prefix.

## Scaling Considerations

SlateDuck's single-writer model means you cannot run multiple writer replicas. For read-only replicas (to distribute read load), you can run additional instances that connect to the same catalog in read-only mode. The writer instance is identified by the highest epoch.

For high availability, use a single replica with aggressive restart policies. Kubernetes will restart the pod within seconds on failure, and the new instance takes over the writer epoch immediately.

## GC CronJob

Schedule garbage collection as a Kubernetes CronJob:

```yaml
apiVersion: batch/v1
kind: CronJob
metadata:
  name: slateduck-gc
spec:
  schedule: "0 3 * * *"  # Daily at 3am
  jobTemplate:
    spec:
      template:
        spec:
          containers:
            - name: gc
              image: ghcr.io/slateduck/slateduck:0.8.0
              command: ["slateduck", "gc", "--storage", "s3://bucket/catalog/", "--retain-days", "30"]
          restartPolicy: OnFailure
          serviceAccountName: slateduck
```
