#!/bin/sh
# Starts Turu without a window (docs/HEADLESS.md, "Docker"). The startup flags come from the
# environment, so an env file holds a whole account; arguments after the image name come after
# them, so they win (`docker run ... turu --exit-on-script-stop`).
#
#   TURU_USER, TURU_PASSWORD     log in (the password stays in the environment, not the command line)
#   TURU_SERVER                  the server to log in on
#   TURU_RUN_SCRIPT              a script to start after the login (TURU_LOAD_SCRIPT: only load it)
#   TURU_RUN_PLAYLIST            a playlist to start (TURU_LOAD_PLAYLIST: only load it)
#   TURU_API                     the control API's address (default 0.0.0.0:9021; empty: off)
#   TURU_MONITOR                 the web monitor's address, e.g. 0.0.0.0:8790
#   TURU_REPORT_TO               report to another web monitor (with TURU_MONITOR_PUSH_KEY)
set -eu

# The arguments given stay first while the flags are added, then move to the end.
given=$#
set -- "$@" --headless
if [ -n "${TURU_USER:-}" ]; then set -- "$@" --user "$TURU_USER"; fi
if [ -n "${TURU_SERVER:-}" ]; then set -- "$@" --server "$TURU_SERVER"; fi
if [ -n "${TURU_RUN_SCRIPT:-}" ]; then set -- "$@" --run-script "$TURU_RUN_SCRIPT"; fi
if [ -n "${TURU_LOAD_SCRIPT:-}" ]; then set -- "$@" --load-script "$TURU_LOAD_SCRIPT"; fi
if [ -n "${TURU_RUN_PLAYLIST:-}" ]; then set -- "$@" --run-playlist "$TURU_RUN_PLAYLIST"; fi
if [ -n "${TURU_LOAD_PLAYLIST:-}" ]; then set -- "$@" --load-playlist "$TURU_LOAD_PLAYLIST"; fi
if [ -n "${TURU_API:-}" ]; then set -- "$@" "--api=$TURU_API"; fi
if [ -n "${TURU_MONITOR:-}" ]; then set -- "$@" "--monitor=$TURU_MONITOR"; fi
if [ -n "${TURU_REPORT_TO:-}" ]; then set -- "$@" --report-to "$TURU_REPORT_TO"; fi
while [ "$given" -gt 0 ]; do
    set -- "$@" "$1"
    shift
    given=$((given - 1))
done

exec /opt/turu/turu "$@"
