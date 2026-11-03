#!/bin/sh
# Test double for the exec source protocol (docs/SOURCES.md). No network.
req=$(cat)
case "$1" in
  list)
    case "$req" in
      *'"search":"jazz"'*) echo '{"stations":[{"id":"jazz","name":"Jazz FM","params":{"genre":"jazz"}}]}' ;;
      *) echo '{"stations":[{"id":"jazz","name":"Jazz FM"},{"id":"news","name":"News","url":"http://news.example/live"},{"id":"","name":"skipped"}]}' ;;
    esac ;;
  resolve)
    case "$req" in
      *'"id":"jazz"'*) echo '{"urls":["http://a.example/jazz","http://b.example/jazz"],"headers":{"X-Key":"k"},"ttl":60}' ;;
      *'"id":"empty"'*) echo '{"urls":[]}' ;;
      *) echo '{"unsupported":true}' ;;
    esac ;;
  now)
    echo '{"artist":"Miles Davis","title":"So What","programme":"Late Jazz"}'
    echo 'not json'
    echo '{"title":"Blue in Green"}'
    sleep 30 ;;
  *) exit 64 ;;
esac
