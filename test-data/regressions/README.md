# Differential regression fixtures

Each subdirectory contains a minimized `input.json`, all solver outputs, and a
short explanation. The Rust workspace tests automatically replay every stored
input. This directory is intentionally empty until a differential failure is
found; generated failures must not be deleted after they are fixed.
