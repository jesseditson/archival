#!/bin/bash

GLOBIGNORE=".."
cd "$(dirname "$0")/archival-website"
rm ../archival-website.zip
zip -r ../archival-website.zip *
cd -
