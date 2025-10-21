#!/bin/sh

RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

function check() {
    name=$1
    command=$2
    sh -c "$command" >/dev/null 2>&1
    result=$?
    if [ $result -eq 0 ]; then
        echo "${GREEN}✅ $name${NC}"
    else
        echo "${RED}❌ $name failed${NC}"
        exit 1
    fi
}

check "fmt" "cargo fmt"
check "check" "cargo check"
check "clippy" "cargo clippy" &
check "machete" "cargo machete" &
check "deny" "cargo deny check licenses" &
check "test" "cargo test" &
check "changelog syntax" "xmllint --noout CHANGELOG.xml" &
check "changelog schema" "xmllint --noout --schema CHANGELOG.xsd CHANGELOG.xml" &
wait
echo "${GREEN}🎉 all done!${NC}"
