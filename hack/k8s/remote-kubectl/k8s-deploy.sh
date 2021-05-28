#! /bin/sh

kubectl create secret generic rospo-secret \
    --from-file=./id_rsa \
    --from-file=./server_key \
    --dry-run=client -o yaml | ubectl apply -f -

kubectl create configmap rospo-config \
    --from-file=./known_hosts \
    --from-file=./rospo.yaml \
    --from-file=./authorized_keys \
    --dry-run=client -o yaml | kubectl apply -f -

kubectl apply -f crb.yaml
kubectl apply -f deployment.yaml
kubectl rollout restart deployment/rospo

sleep 30
kubectl exec -it deployments/rospo -- bash -c '
apt update && apt install -y curl
cd /root
curl -LO "https://dl.k8s.io/release/$(curl -L -s https://dl.k8s.io/release/stable.txt)/bin/linux/amd64/kubectl"
chmod +x kubectl && mv kubectl /usr/local/bin
kubectl config set-cluster cfc --server=https://kubernetes.default --certificate-authority=/var/run/secrets/kubernetes.io/serviceaccount/ca.crt
kubectl config set-context cfc --cluster=cfc
kubectl config set-credentials user --token=$(cat /var/run/secrets/kubernetes.io/serviceaccount/token)
kubectl config set-context cfc --user=user
kubectl config use-context cfc
'