#!/bin/sh
# Run only inside a disposable simulator container. No host trust changes.
set -eu
test -f /.dockerenv
mkdir -p /sim/home /sim/artifacts
test -f /sim/state.json || echo '{}' > /sim/state.json
openssl req -x509 -newkey rsa:2048 -nodes -days 1 \
    -keyout /sim/key.pem -out /sim/cert.pem \
    -subj '/CN=Vitrallis simulator' \
    -addext 'subjectAltName=DNS:api.github.com,DNS:raw.githubusercontent.com' \
    >/sim/certificate.log 2>&1
cp /sim/cert.pem /usr/local/share/ca-certificates/vitrallis-simulator.crt
update-ca-certificates >/sim/trust.log 2>&1
echo '127.0.0.1 api.github.com raw.githubusercontent.com' >> /etc/hosts
Xvfb :99 -screen 0 800x480x24 -nolisten tcp >/sim/xvfb.log 2>&1 &
openbox >/sim/openbox.log 2>&1 &
/usr/bin/python3 tests/simulator/repositories.py >/sim/server.log 2>&1 &
