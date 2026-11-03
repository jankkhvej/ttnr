#!/bin/sh
# `now` that writes far more to stderr than a pipe buffer holds, then a title.
cat >/dev/null
case "$1" in
  now)
    i=0
    while [ $i -lt 2000 ]; do echo "debug line $i ................................................" >&2; i=$((i+1)); done
    echo '{"title":"After the noise"}'
    sleep 30 ;;
  *) exit 64 ;;
esac
