#!/bin/bash
# Bistro benchmark — runs both binaries for N minutes, samples RAM, reports FPS stats.
# Usage: ./bench_mac.sh [duration_seconds]
#
# Prerequisites: both binaries at /tmp/bistro_bench_no_bindless and /tmp/bistro_bench_bindless
# BistroExterior_web.glb in assets/

DURATION=${1:-300}  # default 5 minutes
export RUST_LOG="bevy_diagnostic=info,info"

for name in bistro_bench_no_bindless bistro_bench_bindless; do
    BINARY="./$name"
    LOG="./${name}.log"

    if [ ! -f "$BINARY" ]; then
        echo "ERROR: $BINARY not found. Build both binaries first."
        exit 1
    fi

    echo ""
    echo "==========================================="
    echo "  Running $name for $((DURATION/60))m$((DURATION%60))s"
    echo "==========================================="

    # Start the bench, redirect stderr (where bevy logs go) to file
    "$BINARY" 2>"$LOG" &
    PID=$!

    # Sample RSS memory every 5 seconds
    RAM_SAMPLES=""
    ELAPSED=0
    while [ $ELAPSED -lt $DURATION ] && kill -0 $PID 2>/dev/null; do
        sleep 5
        ELAPSED=$((ELAPSED + 5))
        RSS=$(ps -o rss= -p $PID 2>/dev/null)
        if [ -n "$RSS" ]; then
            RAM_MB=$((RSS / 1024))
            RAM_SAMPLES="$RAM_SAMPLES $RAM_MB"
        fi
    done

    kill $PID 2>/dev/null
    wait $PID 2>/dev/null
    sleep 2

    # Parse FPS avg values, skip first 2 (loading)
    FPS_AVGS=$(grep "fps" "$LOG" | grep -oE 'avg [0-9.]+' | awk '{print $2}' | tail -n +3)
    FPS_COUNT=$(echo "$FPS_AVGS" | wc -l | tr -d ' ')

    if [ "$FPS_COUNT" -gt 0 ] && [ -n "$FPS_AVGS" ]; then
        FPS_STATS=$(echo "$FPS_AVGS" | awk '
            BEGIN { min=99999; max=0; sum=0; n=0 }
            { sum+=$1; n++; if($1<min)min=$1; if($1>max)max=$1 }
            END { printf "Samples: %d\n  Avg:     %.1f\n  Min:     %.1f\n  Max:     %.1f", n, sum/n, min, max }
        ')
        echo "  --- FPS ---"
        echo "  $FPS_STATS"
    else
        echo "  --- FPS ---"
        echo "  No FPS data found"
    fi

    if [ -n "$RAM_SAMPLES" ]; then
        RAM_STATS=$(echo "$RAM_SAMPLES" | tr ' ' '\n' | grep -v '^$' | awk '
            BEGIN { max=0; sum=0; n=0 }
            { sum+=$1; n++; if($1>max)max=$1 }
            END { printf "Avg:     %d MB\n  Peak:    %d MB", sum/n, max }
        ')
        echo "  --- RAM (RSS) ---"
        echo "  $RAM_STATS"
    fi

    echo ""
done

echo "==========================================="
echo "  Done"
echo "==========================================="
