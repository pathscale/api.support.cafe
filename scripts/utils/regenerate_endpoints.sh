#!/bin/bash
set -e
cd "$(dirname "$0")/../.."
endpoint-gen --config-dir config/
cp generated/model.rs src/codegen/model.rs
echo "Regenerated src/codegen/model.rs"
