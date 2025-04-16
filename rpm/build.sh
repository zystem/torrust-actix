#!/bin/sh

cp ../target/release/torrust-actix pkg/opt/torrust-actix
/usr/local/bin/fpm -s dir -t rpm \
  -n torrust-actix \
  -v 1.0.0 \
  --description "Self-hosted BitTorrent tracker torrust-actix" \
  --license "MIT" \
  --url "https://github.com/torrust/torrust" \
  --after-install postinstall.sh \
  -C pkg .