#!/bin/bash
set -euo pipefail

run_args=(--rm)
default_command=(--help)

if [[ -t 0 && -t 1 ]]; then
  run_args=(-it --rm)
  default_command=(interactive)
fi

if [[ "$#" -gt 0 ]]; then
  command_args=("$@")
else
  command_args=("${default_command[@]}")
fi

docker run "${run_args[@]}" \
  -v "$(pwd)":/app \
  -w /app \
  agnusdei1207/bco:latest "${command_args[@]}"
