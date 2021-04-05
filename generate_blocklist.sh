#!/bin/bash

function import_hosts() {
    local url=$1
    curl "$url" | sed '/^#/d' | sed 's/0.0.0.0 //g' | sed 's/127.0.0.1 //g' | grep '\S'
}

# In blocklist_config, put a list of URLs to shared ad-blocking hosts files
# e.g.
# URL=(
#    "https://some-domain/hosts"
#)
. ./blocklist_config.sh

echo "" > blocklist.txt
for url in ${URLS[@]}; do
    echo "Importing $url"
    import_hosts $url >> blocklist.txt
done