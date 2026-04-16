#!/bin/bash

set -e

########################################
# ⚙️ CONFIG
########################################
NAMESPACE="chaos-$(date +%s)"   # unique namespace every run
LABEL_KEY="chaos"
LABEL_VALUE="true"
SELECTOR="$LABEL_KEY=$LABEL_VALUE"

PIDS=()

########################################
# 🛑 CLEANUP HANDLER
########################################
cleanup() {
  echo ""
  echo "🛑 Termination signal received. Cleaning up..."

  # Kill all background jobs
  for PID in "${PIDS[@]}"; do
    if kill -0 "$PID" 2>/dev/null; then
      echo "🔪 Killing process $PID"
      kill -9 "$PID" 2>/dev/null || true
    fi
  done

  echo "🧹 Deleting namespace: $NAMESPACE"
  kubectl delete ns $NAMESPACE --ignore-not-found >/dev/null 2>&1 || true

  echo "✅ Cleanup complete"
  exit 0
}

trap cleanup SIGINT SIGTERM

########################################
# 📦 SETUP
########################################
echo "📦 Creating namespace: $NAMESPACE"
kubectl create ns $NAMESPACE

echo "📦 Deploying resources in $NAMESPACE..."

kubectl apply -n $NAMESPACE -f - <<EOF
apiVersion: apps/v1
kind: Deployment
metadata:
  name: web-app
  labels:
    $LABEL_KEY: "$LABEL_VALUE"
spec:
  replicas: 5
  selector:
    matchLabels:
      app: web-app
      $LABEL_KEY: "$LABEL_VALUE"
  template:
    metadata:
      labels:
        app: web-app
        $LABEL_KEY: "$LABEL_VALUE"
    spec:
      containers:
      - name: nginx
        image: nginx:latest
        ports:
        - containerPort: 80
---
apiVersion: v1
kind: Service
metadata:
  name: web-service
  labels:
    $LABEL_KEY: "$LABEL_VALUE"
spec:
  selector:
    app: web-app
    $LABEL_KEY: "$LABEL_VALUE"
  ports:
    - port: 80
      targetPort: 80
EOF

echo "⏳ Waiting for deployment..."
kubectl wait -n $NAMESPACE --for=condition=available --timeout=60s deployment/web-app

echo "🔥🔥 CHAOS STARTED in namespace: $NAMESPACE 🔥🔥"

########################################
# 💥 POD CHAOS
########################################
pod_chaos() {
  while true; do
    PODS=$(kubectl get pods -n $NAMESPACE -l $SELECTOR -o name)

    echo "💥 Killing ALL pods"
    for POD in $PODS; do
      kubectl delete -n $NAMESPACE $POD --force --grace-period=0 >/dev/null 2>&1 &
    done

    sleep 2

    for i in {1..5}; do
      POD=$(kubectl get pods -n $NAMESPACE -l $SELECTOR -o name | shuf -n1)
      [ ! -z "$POD" ] && kubectl delete -n $NAMESPACE $POD --force --grace-period=0 >/dev/null 2>&1 &
      sleep 0.5
    done

    sleep 2
  done
}

########################################
# 📉 DEPLOYMENT CHAOS
########################################
deploy_chaos() {
  while true; do
    SCALE=$((RANDOM % 15))
    echo "📉 Scaling to $SCALE"
    kubectl scale deployment web-app -n $NAMESPACE --replicas=$SCALE >/dev/null 2>&1

    sleep 2

    echo "📉 Scaling to 0"
    kubectl scale deployment web-app -n $NAMESPACE --replicas=0 >/dev/null 2>&1

    sleep 2

    echo "📈 Scaling to 10"
    kubectl scale deployment web-app -n $NAMESPACE --replicas=10 >/dev/null 2>&1

    sleep 3
  done
}

########################################
# 🔄 DEPLOYMENT RECREATE CHAOS
########################################
deploy_recreate_chaos() {
  while true; do
    echo "🔥 Deleting deployment"
    kubectl delete deployment web-app -n $NAMESPACE --ignore-not-found >/dev/null 2>&1

    sleep 2

    echo "♻️ Recreating deployment"
    kubectl apply -n $NAMESPACE -f - <<EOF >/dev/null 2>&1
apiVersion: apps/v1
kind: Deployment
metadata:
  name: web-app
  labels:
    $LABEL_KEY: "$LABEL_VALUE"
spec:
  replicas: 5
  selector:
    matchLabels:
      app: web-app
      $LABEL_KEY: "$LABEL_VALUE"
  template:
    metadata:
      labels:
        app: web-app
        $LABEL_KEY: "$LABEL_VALUE"
    spec:
      containers:
      - name: nginx
        image: nginx:latest
EOF

    sleep 5
  done
}

########################################
# 🔌 SERVICE CHAOS
########################################
service_chaos() {
  while true; do
    echo "🔌 Breaking service"
    kubectl patch svc web-service -n $NAMESPACE \
      -p '{"spec":{"selector":{"app":"broken"}}}' >/dev/null 2>&1

    sleep 1

    echo "🔥 Deleting service"
    kubectl delete svc web-service -n $NAMESPACE --ignore-not-found >/dev/null 2>&1

    sleep 1

    echo "♻️ Recreating service"
    kubectl apply -n $NAMESPACE -f - <<EOF >/dev/null 2>&1
apiVersion: v1
kind: Service
metadata:
  name: web-service
  labels:
    $LABEL_KEY: "$LABEL_VALUE"
spec:
  selector:
    app: web-app
    $LABEL_KEY: "$LABEL_VALUE"
  ports:
    - port: 80
      targetPort: 80
EOF

    sleep 2
  done
}

########################################
# 🚀 START CHAOS
########################################

pod_chaos & PIDS+=($!)
deploy_chaos & PIDS+=($!)
deploy_recreate_chaos & PIDS+=($!)
service_chaos & PIDS+=($!)

wait "${PIDS[@]}"