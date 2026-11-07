{
  perSystem =
    { pkgs, ... }:
    let
      rust-toolchain = pkgs.rust-bin.stable.latest.default.override {
        extensions = [
          "rust-analyzer"
          "rust-src"
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
