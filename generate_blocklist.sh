#!/bin/bash

function import_hosts() {
    local url=$1
    # Trim lines, remove comments, convert tabs to spaces, collapse multiple spaces, and
    # take the second column of the output
    curl "$url"| sed -E 's/^[ \t]+//g' | sed '/^#/d' | sed 's/\t/ /g' | sed -E 's/[ ]+/ /g' | cut -d ' ' -f 2 | grep '\S'
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