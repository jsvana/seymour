#!/bin/bash

# Read from .env file
set -o allexport; source .env; set +o allexport

DATABASE_FILE=${DATABASE_URL#"sqlite://"}

if [[ -f "$DATABASE_FILE" ]]; then
  if [[ "$1" = "--destroy-data" ]]; then
    echo "Database $DATABASE_FILE exists. Destroying it because --destroy-data was passed."
    rm $DATABASE_FILE
  else
    echo "Database $DATABASE_FILE exists. If you're sure you want to delete it, re-run with --destroy-data."
    exit 1
  fi
fi

sqlx database create
sqlx migrate run

sqlite3 "$DATABASE_FILE" < scripts/test.sql
