dist:
    nix build .#dist
    cp -f result/* dist/

publish-crates-io:
    cargo publish -p resharp-grep

