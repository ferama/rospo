#! /bin/sh

kubectl create configmap rospo-config \
    --from-file=./authorized_keys \
    --from-file=./id_rsa \
    --from-file=./known_hosts \
    --from-file=./rospo.yaml \
    --from-file=./server_key \
    --dry-run -o yaml | kubectl apply -f -

kubectl apply -f deployment.yaml