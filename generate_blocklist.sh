#!/bin/bash

function import_hosts() {
    local url=$1
    curl "$url" | sed '/^#/d' | sed 's/0.0.0.0 //g' | sed 's/127.0.0.1 //g' | grep '\S'
}

URLS=(
    "https://someonewhocares.org/hosts/zero/hosts"
    "https://adaway.org/hosts.txt"
)

echo "" > blocklist.txt
for url in ${URLS[@]}; do
    echo "Importing $url"
    import_hosts $url >> blocklist.txt
done