#!/bin/sh
set -e

# Create user if it doesn't exist
getent group torrust-actix  > /dev/null || /usr/sbin/groupadd -r torrust-actix
getent passwd torrust-actix > /dev/null || /usr/sbin/useradd  -r -g torrust-actix -d / -s /sbin/nologin -c "torrust-actix User" torrust-actix

# Set permissions
chown -R torrust-actix:torrust-actix /opt/torrust-actix

# Reload and enable service
systemctl daemon-reload || true
systemctl enable torrust-actix.service || true
