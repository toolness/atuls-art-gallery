#! /usr/bin/env sh

set -e

export DB_NAME=initial-db.sqlite

rm -f ${DB_NAME}

cd rust
cargo run --release -- --db-path=../${DB_NAME} csv
cargo run --release -- --db-path=../${DB_NAME} layout --clear
