#! /bin/bash

# This is an example script that runs on MacOS only (conversion to linux should be trivial)
# It runs `rospo` as a SOCKS5 and DNS proxy
# It runs `tun2socks` to route all the traffic through the rospo proxies

# Ensure you have tun2socks installed (https://github.com/xjasonlyu/tun2socks)
# Ensure you have rospo >= 0.13 installed (https://github.com/ferama/rospo)
# Configure the section below
# run.

# this is the target host
# all the traffic will be proxied through this host
SSH_HOST="[put your ssh host here]"
SSH_PORT="[put your ssh port here]"

REMOTE_DNS="1.1.1.1:53"

############################################################################
GATEWAY=`route -n get default | grep gateway | awk '{print $2}'`
GATEWAY6=`route -n get -inet6 default | grep gateway | awk '{print $2}'`
TMPFILE=$(mktemp -p /tmp)

egress() {
    sudo route delete $SSH_HOST
    sudo route add default $GATEWAY
    sudo route add -inet6 default $GATEWAY6
    networksetup -setdnsservers Wi-Fi empty
    rm $TMPFILE
}
trap egress EXIT

run_rospo() {

    cat > $TMPFILE <<EOF
sshclient:
  server: $SSH_HOST:$SSH_PORT

socksproxy:
  listen_address: :1080

dnsproxy:
  listen_address: :53
  remote_dns_address: $REMOTE_DNS
EOF
    
    rospo run $TMPFILE &
}

run_tun() {
    echo "Starting tun2socks..."
    sudo tun2socks \
        -device utun123 \
        -proxy socks5://127.0.0.1:1080 \
        -interface en0 \
        -tcp-auto-tuning &


    until ifconfig | grep utun123;
    do
        sleep 1
        echo "working..."
    done

    sudo ifconfig utun123 198.18.0.1 198.18.0.1 up

    sudo route delete default
    sudo route delete -inet6 default


    sudo route add default -interface utun123
    sudo route add $SSH_HOST $GATEWAY

    networksetup -setdnsservers Wi-Fi 127.0.0.1
}

run_rospo
run_tun

# wait forever
tail -f /dev/null