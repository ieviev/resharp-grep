dist:
    nix build .#dist
    cp -f result/* dist/
