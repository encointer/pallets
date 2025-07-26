export RUSTFLAGS="$RUSTFLAGS --cfg substrate_runtime"

find . -name "Cargo.toml" | while read -r CARGO_TOML; do
  DIR=$(dirname "$CARGO_TOML")
  echo "Checking in directory: $DIR"

  # Skip the loop if the crate does not have a feature `std`
  if ! grep -q "\[features\]" "$CARGO_TOML" || ! grep -q "std = \[" "$CARGO_TOML"; then
      echo "Feature 'std' not found in $CARGO_TOML. Skipping."
      continue
  fi

  if grep -q "\[features\]" "$CARGO_TOML" && grep -q "runtime-benchmarks = \[" "$CARGO_TOML"; then
      echo "Feature 'runtime-benchmarks' found, adding this feature."
      cargo $COMMAND $@ --features runtime-benchmarks --manifest-path "$CARGO_TOML"
  else
      echo "Feature 'runtime-benchmarks' not found, running command without this feature"
      cargo $COMMAND $@ --manifest-path "$CARGO_TOML";
  fi
done
