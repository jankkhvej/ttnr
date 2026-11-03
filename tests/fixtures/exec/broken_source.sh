#!/bin/sh
# Fails `list` with a message, implements nothing else (exit 64).
cat >/dev/null
case "$1" in
  list) echo "cannot reach the station directory" >&2; exit 2 ;;
  *) exit 64 ;;
esac
