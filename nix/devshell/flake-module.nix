{
  perSystem =
    { pkgs, ... }:
    let
      rust-toolchain = pkgs.rust-bin.stable.latest.default.override {
        extensions = [
          "rust-analyzer"
          "rust-src"
        ];
        targets = [
          "wasm32-unknown-unknown"
        ];
      };
    in
    {
      devShells.default = pkgs.mkShell {
        packages = builtins.attrValues {
          inherit rust-toolchain;
          inherit (pkgs)
            openssl
            ;
        };
      };
    };
}
