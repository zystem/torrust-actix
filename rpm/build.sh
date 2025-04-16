#!/bin/sh

cp ../target/release/torrust-actix pkg/opt/torrust-actix
docker build -t fpm .
docker run -v $(pwd):/src fpm -s dir -t rpm \
  -n torrust-actix \
  -v 4.0.10 \
  --description "Torrust-Actix Tracker is a lightweight but incredibly powerful and feature-rich BitTorrent Tracker made using Rust." \
  --license "MIT" \
  --url "https://github.com/torrust/torrust" \
  --after-install postinstall.sh \
  -C pkg .