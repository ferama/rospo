#! /bin/sh

# take this script as a sample.

kubectl create configmap rospo-config \
    --from-file=./id_rsa \
    --from-file=./known_hosts \
    --from-file=./rospo.yaml \
    --dry-run=client -o yaml | kubectl apply -f -

kubectl apply -f deployment.yaml
kubectl rollout restart deployment/rospo