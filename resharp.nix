{
  lib,
  installShellFiles,
  rustPlatform,
}:
rustPlatform.buildRustPackage {
  pname = "re";
  version = "0.3.0";
  src = lib.cleanSource ./.;
  cargoLock.lockFile = ./Cargo.lock;

  nativeBuildInputs = [ installShellFiles ];

  postInstall = ''
    ln -s $out/bin/re $out/bin/resharp
    installShellCompletion --cmd re \
      --bash <($out/bin/re --completions bash) \
      --zsh <($out/bin/re --completions zsh) \
      --fish <($out/bin/re --completions fish)
  '';

  meta = {
    description = "grep tool powered by the resharp regex engine with intersection, complement, and lookarounds";
    license = lib.licenses.mit;
    mainProgram = "re";
  };
}
